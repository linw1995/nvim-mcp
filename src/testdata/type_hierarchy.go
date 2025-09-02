package main

import "fmt"

// Base interface - will be supertype of derived interfaces
type Shape interface {
	Area() float64
	Perimeter() float64
}

// Derived interface - inherits from Shape
type ColoredShape interface {
	Shape
	Color() string
}

// Another derived interface - inherits from Shape
type NamedShape interface {
	Shape
	Name() string
}

// Concrete type implementing Shape
type Circle struct {
	Radius float64
}

func (c Circle) Area() float64 {
	return 3.14159 * c.Radius * c.Radius
}

func (c Circle) Perimeter() float64 {
	return 2 * 3.14159 * c.Radius
}

// Concrete type implementing ColoredShape
type ColoredCircle struct {
	Circle
	ColorValue string
}

func (cc ColoredCircle) Color() string {
	return cc.ColorValue
}

// Concrete type implementing both interfaces
type Rectangle struct {
	Width  float64
	Height float64
}

func (r Rectangle) Area() float64 {
	return r.Width * r.Height
}

func (r Rectangle) Perimeter() float64 {
	return 2 * (r.Width + r.Height)
}

// Another concrete type implementing NamedShape
type NamedRectangle struct {
	Rectangle
	ShapeName string
}

func (nr NamedRectangle) Name() string {
	return nr.ShapeName
}

func main() {
	// Use the types to demonstrate hierarchy
	shapes := []Shape{
		Circle{Radius: 5},
		Rectangle{Width: 10, Height: 5},
	}

	for _, shape := range shapes {
		fmt.Printf("Area: %.2f, Perimeter: %.2f\n", shape.Area(), shape.Perimeter())
	}
}
