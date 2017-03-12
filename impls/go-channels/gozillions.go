// Steve Phillips / elimisteve
// 2017.03.12

package main

import (
	"fmt"
	"log"
	"net"
	"os"
	"sync"
)

func main() {
	listenAddr := parseAddr()
	fmt.Printf("listening on %s\n", listenAddr)

	l, err := net.Listen("tcp", listenAddr)
	if err != nil {
		log.Fatalf("Error listening on %s: %s", listenAddr, err)
	}
	defer l.Close()

	cm := NewConnectionManager()

	for {
		conn, err := l.Accept()
		if err != nil {
			log.Printf("Error accepting connection: %s\n", err)
			continue
		}
		go handle(cm, conn)
	}
}

func parseAddr() string {
	addr := "127.0.0.1:8000"
	if len(os.Args) > 1 {
		addr = os.Args[1]
	}
	return addr
}

func handle(cm *ConnectionManager, conn net.Conn) {
	cm.Add <- conn

	buf := make([]byte, 1)
	_, err := conn.Read(buf)
	if err != nil {
		log.Printf("Error reading initial byte: %s\n", err)
		return
	}

	header := buf[0]

	buf = make([]byte, int(header))
	n, err := conn.Read(buf)
	if err != nil {
		log.Printf("Error reading message bytes: %s\n", err)
		return
	}

	cm.Broadcast <- append([]byte{header}, buf[:n]...)
}

type ConnectionManager struct {
	Add       chan net.Conn
	Broadcast chan []byte
}

func (cm *ConnectionManager) loop() {
	var conns []net.Conn
	wg := &sync.WaitGroup{}

	for {
		select {
		case c := <-cm.Add:
			conns = append(conns, c)
		case msg := <-cm.Broadcast:
			wg.Add(len(conns))
			log.Printf("Iterating over %d conns\n", len(conns))
			for i, c := range conns {
				if c == nil {
					wg.Done()
					continue
				}
				go func(conn net.Conn) {
					_, err := conn.Write(msg)
					if err != nil {
						log.Printf("Error writing to conn: %s\n", err)

						// TODO: Consider doing more checks to ensure
						// that I don't need to call c.Close() here

						// TODO: Fix this (small) memory leak
						conns[i] = nil
					}
					wg.Done()
				}(c)
			}
			wg.Wait()
		}
	}
}

func NewConnectionManager() *ConnectionManager {
	cm := &ConnectionManager{
		Add:       make(chan net.Conn),
		Broadcast: make(chan []byte),
	}
	go cm.loop()
	return cm
}
