# Light control

This repository contains a small client/server architecture I use to control my "smart" IKEA lights.


## Ideas
- use a client/server architecture where the server is a systemd service maintaining a connection to the mqtt broker and a client binary which sends messages  