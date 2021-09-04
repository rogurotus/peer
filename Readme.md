# Starting the first peer with messaging period 5 seconds at port 8080:
./peer --period=5 --port=8080

# Starting the second peer which will connect to the first
# messaging period - 6 seconds
# port - 8081
./peer --period=6 --port=8081 --connect="127.0.0.1:8080"

where period - integer

connect only takes 1 argument

messages are broadcast excluding the source of the message