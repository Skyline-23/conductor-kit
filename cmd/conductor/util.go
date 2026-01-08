package main

import (
	"encoding/json"
	"fmt"
	"math/rand"
	"os"
	"strings"
	"time"
)

func getenv(key, fallback string) string {
	if val := os.Getenv(key); val != "" {
		return val
	}
	return fallback
}

func pathExists(p string) bool {
	_, err := os.Stat(p)
	return err == nil
}

func splitList(val string) []string {
	parts := strings.Split(val, ",")
	out := []string{}
	for _, p := range parts {
		p = strings.TrimSpace(p)
		if p != "" {
			out = append(out, p)
		}
	}
	return out
}

func printJSON(payload map[string]interface{}) {
	out, _ := json.MarshalIndent(payload, "", "  ")
	fmt.Println(string(out))
}

func randomSeed() {
	rand.Seed(time.Now().UnixNano())
}

func init() {
	randomSeed()
}
