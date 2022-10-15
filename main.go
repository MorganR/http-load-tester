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
	host           = flag.String("host", "", "The host to connect to. Must include the scheme.")
	paths          = flag.String("paths", "", "Backslash (\\) separated paths to query.")
	pathsFile      = flag.String("paths_file", "", "The file to read URL paths from, one per line.")
	maxConcurrency = flag.Int("c", 10, "Max concurrency to use in the load test.")
	rampStyle      = flag.String("ramp_style", "doubling", "Determines how concurrency ramps. Either 'linear' or 'doubling'.")
	linearRampStep = flag.Int("linear_ramp_step", 5, "The amount that concurrency increases at each stage. Only applies if ramp_style is linear.")
	stageDelay     = flag.Duration("stage_delay", 10*time.Second, "How long to send requests at each degree of concurrency.")
	errorThreshold = flag.Float64("err_threshold", 0.05, "The error rate at which the stress test will be canceled, even if the max concurrency has not yet been reached.")
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
	if *errorThreshold <= 0 || *errorThreshold > 1.0 {
		log.Fatalf("err_threshold must be > 0 and <= 1.0. Received %.3f", *errorThreshold)
	}

	tester := load.NewTester(concurrencyCap)
	err = tester.Init(urls)
	if err != nil {
		log.Fatalf("Failed to init the tester: %v", err.Error())
	}

	concurrency := 2
	lastConcurrency := 1
	shouldContinue := true
	for ; concurrency <= concurrencyCap && shouldContinue; concurrency = increaseConcurrency(concurrency) {
		shouldContinue = stressTestWithConcurrency(concurrency, tester)
		lastConcurrency = concurrency
	}
	if shouldContinue && lastConcurrency != concurrencyCap {
		// Run one more at the cap, if the cap is not a multiple of 2
		stressTestWithConcurrency(concurrencyCap, tester)
	}
}

func increaseConcurrency(current int) int {
	switch *rampStyle {
	case "linear":
		return current + *linearRampStep
	case "doubling":
		return current + current
	default:
		log.Fatalf("ramp_style must be set to a valid value (linear or doubling). Received: %v", *rampStyle)
		return 1
	}
}

// Returns true if the test should continue.
func stressTestWithConcurrency(concurrency int, tester *load.Tester) bool {
	ctx, cancel := context.WithTimeout(context.Background(), *stageDelay)
	result, err := tester.Stress(ctx, concurrency)
	if err != nil {
		log.Fatalf("Stress test failed at concurrency %d: %v", concurrency, err.Error())
	}
	cancel()
	log.Printf("Result at concurrency %v\n%v\nDetails:\n%s", concurrency, result.SummaryString(), result)
	numSuccess := int64(0)
	numFailures := int64(0)
	for _, r := range result.ResultsByUrl {
		numSuccess += r.Successes.NumCalls
		numFailures += r.Failures.NumCalls
	}
	if numSuccess == 0 {
		log.Printf("No successful calls at concurrency %v", concurrency)
		return false
	}
	errRate := float64(numFailures) / float64(numSuccess)
	if errRate > *errorThreshold {
		log.Printf("Error rate over threshold at concurrency %v. Rate: %.3f", concurrency, errRate)
		return false
	}
	return true
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
