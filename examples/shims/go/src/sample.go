package main

import (
	"fmt"
	"os"
)

func main() {
	args := os.Args[1:]
	fmt.Println("sample: args", args)

	if len(args) == 0 || args[0] != "greet" {
		fmt.Println("Unknown command")
		os.Exit(1)
	}

	var msg string
	if len(args) > 1 {
		msg = args[1]
	}
	greet(msg)
}

func greet(msg string) {
	fmt.Println("sample: greet", msg)
	switch msg {
	case "hello":
		fmt.Println("Hello, World!")
	case "goodbye":
		fmt.Println("Goodbye, World!")
	default:
		fmt.Println("sample: Unknown command")
		os.Exit(2)
	}
}
