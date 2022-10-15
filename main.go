package main

import (
	"bufio"
	"context"
	"flag"
	"fmt"
	"log"
	"net/url"
	"os"
	"strings"
	"time"

	"github.com/MorganR/http-load-tester/load"
)

const pathSeparator = "\\"

var (
	host           = flag.String("host", "", "The host to connect to.")
	paths          = flag.String("paths", "", "Backslash (\\) separated paths to query.")
	pathsFile      = flag.String("paths_file", "", "The file to read URL paths from, one per line.")
	maxConcurrency = flag.Int("c", 10, "Max concurrency to use in the load test.")
	stageDelay     = flag.Duration("stage_delay", 10*time.Second, "How long to send requests at each degree of concurrency.")
)

const absoluteMaxConcurrency = 512

func main() {
	flag.Parse()

	if *host == "" {
		log.Fatal("A value for host must be provided.")
	}

	urls, err := constructURLs(*host, strings.Split(*paths, pathSeparator))
	if err != nil {
		log.Fatalf("Failed to construct urls from paths flag. Error: %v", err.Error())
	}
	if *pathsFile != "" {
		moreUrls, err := loadAndValidateURLsFromFile(*host, *pathsFile)
		if err != nil {
			log.Fatalf("Failed to load urls: %v", err.Error())
		}
		urls = append(urls, moreUrls...)
	}
	concurrencyCap := *maxConcurrency
	if concurrencyCap > absoluteMaxConcurrency {
		concurrencyCap = absoluteMaxConcurrency
		log.Printf("Capping concurrency at %v", concurrencyCap)
	}

	tester := load.NewTester(concurrencyCap)
	err = tester.Init(urls)
	if err != nil {
		log.Fatalf("Failed to init the tester: %v", err.Error())
	}

	concurrency := 2
	for ; concurrency <= concurrencyCap; concurrency += concurrency {
		stressTestWithConcurrency(concurrency, tester)
	}
	if concurrency/2 != concurrencyCap {
		// Run one more at the cap, if the cap is not a multiple of 2
		stressTestWithConcurrency(concurrencyCap, tester)
	}
}

func stressTestWithConcurrency(concurrency int, tester *load.Tester) {
	ctx, cancel := context.WithTimeout(context.Background(), *stageDelay)
	result, err := tester.Stress(ctx, concurrency)
	if err != nil {
		log.Fatalf("Stress test failed at concurrency %d: %v", concurrency, err.Error())
	}
	cancel()
	log.Printf("Result at concurrency %v\n%s", concurrency, result)
}

func constructURLs(host string, paths []string) ([]string, error) {
	urls := make([]string, 0, len(paths))
	for _, p := range paths {
		u, err := url.Parse(host + p)
		if err != nil {
			return nil, fmt.Errorf("failed to parse URL; %v", err.Error())
		}
		urls = append(urls, u.String())
	}
	return urls, nil
}

func loadAndValidateURLsFromFile(host, filename string) ([]string, error) {
	f, err := os.Open(filename)
	if err != nil {
		return nil, fmt.Errorf("failed to open file %v: %v", filename, err.Error())
	}
	s := bufio.NewScanner(f)
	urls := make([]string, 0)
	for s.Scan() {
		l := s.Text()
		u, err := url.Parse(host + l)
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
