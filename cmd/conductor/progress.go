package main

type progressReporter func(message string, progress float64, total float64)
