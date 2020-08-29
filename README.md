# Light control

This repository contains a small client/server architecture I use to control my "smart" IKEA lights.  

The project consists of a shared library and two binaries - the server and the client. The server is intended to run as a systemd service, maintaing a connection to the MQTT broker and listening for messages on a Unix Domain Socket. The client provides a simple CLI so that I can call it from my window manager on a keypress, it connects to the server and send a message via the Unix socket. The server running as a systemd service then transforms this message into one for the MQTT broker.  
The main reason for this is, that connecting to the MQTT broker sometimes takes upwards of one second and that is to much latency to do on every light change.
 