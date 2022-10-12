package main

import (
	"bufio"
	"flag"
	"fmt"
	"log"
	"net/url"
	"os"
)

var (
	urlsFile = flag.String("urls_file", "", "The file to read URLs from, one per line.")
)

func main() {
	flag.Parse()

	if urlsFile == nil || *urlsFile == "" {
		log.Fatalf("URLs must be provided.")
	}
	urls, err := loadAndValidateURLs(*urlsFile)
	if err != nil {
		log.Fatalf("Failed to load urls: %v", err.Error())
	}

	for _, u := range urls {
		log.Printf("Testing url %v", u)
		// TODO: Use the URL
	}
}

func loadAndValidateURLs(filename string) ([]string, error) {
	f, err := os.Open(filename)
	if err != nil {
		return nil, fmt.Errorf("failed to open file %v: %v", filename, err.Error())
	}
	s := bufio.NewScanner(f)
	urls := make([]string, 0)
	for s.Scan() {
		l := s.Text()
		u, err := url.Parse(l)
		if err != nil {
			return nil, fmt.Errorf("could not parse url %v. Error: %v", l, err.Error())
		}
		urls = append(urls, u.String())
	}
	if s.Err() != nil {
		return nil, s.Err()
	}
	return urls, nil
}
