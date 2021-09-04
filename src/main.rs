
use std::{collections::HashMap, env, mem::swap};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::{TcpListener, TcpStream, ToSocketAddrs, tcp::OwnedWriteHalf}, time::sleep};
use std::sync::{Arc};
use tokio::sync::Mutex;
use tokio::sync::mpsc::*;
use std::time::Duration;
use std::sync::atomic::*;
use std::sync::atomic::Ordering;
use std::time::Instant;

static ID: AtomicUsize = AtomicUsize::new(1);
fn get_id() -> usize
{
    ID.fetch_add(1, Ordering::Relaxed)
}


struct Peer
{
    peers: Arc<Mutex<Vec<(usize, OwnedWriteHalf)>>>,
    messages: Receiver<(usize, String)>,
    send: Sender<(usize, String)>,
    period: Duration,
    time: Instant,
}

impl Peer
{
    async fn run_loop(mut self)
    {
        while let Some((skip_id, i)) = self.messages.recv().await
        {
            let data = i.as_bytes();
            let mut vec = self.peers.lock().await;
            let mut temp = Vec::with_capacity(vec.len());

            swap(&mut *vec, &mut temp);
            let out: Vec<String> = temp
                .iter()
                .filter(|a| a.0 != skip_id)
                .map(|(_,ref a)|
            {
                format!("{:?}", a)
                    .split_terminator("peer: ")
                    .nth(1).unwrap()
                    .split_terminator(',')
                    .next().unwrap().to_owned()
            }).collect();

            println!("time:{} - Sending message send[{}] to {:?}", self.time.elapsed().as_secs(), &i, out);
            for mut i in temp.into_iter()
            {
                if i.0 == skip_id {vec.push(i);}
                else
                {
                    match i.1.write(data).await
                    {
                        Ok(_) => vec.push(i),
                        Err(_) => println!("time:{} - disconnect [{}]", 
                            self.time.elapsed().as_secs(),                
                            format!("{:?}", i.1)
                            .split_terminator("peer: ")
                            .nth(1).unwrap()
                            .split_terminator(',')
                            .next().unwrap().to_owned()),
                    }
                }
            }
        }
    }

    async fn connect<T: ToSocketAddrs>(&mut self, addr: T)
    {
        let conn = TcpStream::connect(addr).await.unwrap();
        let peers = self.peers.clone();
        let rx = self.send.clone();
        let time = self.time;
        tokio::spawn(async move 
        {
            let id = get_id();
            let mut lock = peers.lock().await;
            lock.push((id, Peer::process(conn, rx, id, time).await));
        });
    }

    async fn listen(&mut self, tcp: TcpListener)
    {
        let peers = self.peers.clone();
        let send = self.send.clone();
        let time = self.time;
        tokio::spawn( async move 
        {
            loop 
            {
                let (socket, _) = tcp.accept().await.unwrap();
                let mut lock = peers.lock().await;
                let id = get_id();
                lock.push((id, Peer::process(socket, send.clone(), id, time).await));
            }
        });
    }
    
    async fn random_message(&mut self)
    {
        let period = self.period;
        let send = self.send.clone();
        tokio::spawn(async move 
        {
            loop
            {
                sleep(period).await;
                send.send((0,format!("random from {:?} period", period))).await.unwrap();
            }
        });
    }

    async fn run<T: ToSocketAddrs>(period: Duration, connect: Option<T>, port: &str)
    {
        let (send, messages) = channel(1);
        let mut p = Peer 
            { 
                peers: Arc::new(Mutex::new(vec![])), 
                messages, 
                send, 
                period,
                time: std::time::Instant::now(),
            };
        
        if let Some(addr) = connect
        {
            p.connect(addr).await;
        }
        let tcp = TcpListener::bind(
                &format!("0.0.0.0:{}", port)
            ).await.expect("cannot connect");
        p.listen(tcp).await;
        p.random_message().await;
        p.run_loop().await;
    }

    async fn process(socket: TcpStream, send: Sender<(usize, String)>, id: usize, time: Instant) -> OwnedWriteHalf
    {
        let (mut read, write) = socket.into_split();
        let addr = format!("{:?}", &write)
            .split_terminator("peer: ")
            .nth(1).unwrap()
            .split_terminator(',')
            .next().unwrap().to_owned();

        tokio::spawn( async move 
        {
            let mut buf = vec![0;64];
            'dead: loop
            {
                loop 
                {
                    match read.read(&mut buf).await
                    {
                        Ok(n) => 
                        {
                            if n == 0 
                            {
                                break;
                            }
                            else 
                            {
                                println!("time:{} - Received message [{}] from {}"
                                    ,time.elapsed().as_secs()
                                    ,&String::from_utf8_lossy(&buf[..n])
                                    ,addr
                                );
                                send.send((id, String::from_utf8_lossy(&buf[..n]).into_owned())).await.unwrap();
                                buf.clear();
                            }
                        },
                        _ => break 'dead
                    }
                }
            }
        });
        write
    }
}



#[tokio::main]
async fn main()
{
    let args= env::args();
    let args: HashMap<String, String> =
    {
        let mut hashmap = HashMap::new();
        for i in args.into_iter().skip(1)
            .map(|a| a.split_terminator('=').map(|a| a.to_owned())
            .collect::<Vec<String>>())
            .collect::<Vec<Vec<String>>>().into_iter()
        {
            hashmap.insert(i[0].clone(), i[1].clone());
        }
        hashmap
    };

    let conn = args.get("--connect");

    let period = Duration::from_secs(
        args.get("--period")
        .expect("no period in args")
        .parse().expect("period is not number"));

    let port = args.get("--port").expect("no port in args");

    Peer::run(period, conn, port).await;
}