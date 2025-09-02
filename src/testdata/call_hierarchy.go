package main

import "fmt"

// Helper function that will be called by others
func helper() {
	fmt.Println("helper function")
}

// Utility function that calls helper
func utility() {
	fmt.Println("utility function")
	helper()
}

// Main caller that calls both utility and helper
func caller() {
	fmt.Println("caller function")
	utility()
	helper()
}

// Another function that calls caller
func mainFunc() {
	fmt.Println("main function")
	caller()
}

// Entry point
func main() {
	mainFunc()
}
