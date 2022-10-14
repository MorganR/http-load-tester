package load

import (
	"context"
	"fmt"
	"log"
	"math/rand"
	"sort"
	"strings"
	"time"

	"github.com/valyala/fasthttp"
	"golang.org/x/exp/maps"
	"golang.org/x/sync/errgroup"
)

const bodyLengthAllowedChange = 10
const bufferSize = 16 << 10

// Tester tests some URLs, performing basic validation as it goes.
type Tester struct {
	urls          []string
	responseByUrl map[string]expectedResponseData
	client        *fasthttp.Client
}

// StressResult returns the results of a stress test.
type StressResult struct {
	ResultsByUrl map[string]*ResultWithValidity
}

// ResultWithValidity provides separate results for sucessful and failed fetches.
type ResultWithValidity struct {
	Successes AggregateResult
	Failures  AggregateResult
}

// AggregateResult provides the aggregate result data for a set of fetches.
type AggregateResult struct {
	NumCalls           int64
	TotalBytesReceived int64
	TotalLatency       time.Duration
}

type urlResult struct {
	isValid       bool
	bytesReceived int
	latency       time.Duration
}

type expectedResponseData struct {
	StatusCode int
	MinLength  int
	MaxLength  int
}

// NewTester constructs a new tester object.
func NewTester(maxConcurrency int) *Tester {
	return &Tester{
		client: &fasthttp.Client{
			Name:            "http-load-tester",
			MaxConnsPerHost: maxConcurrency,
			// Don't retry because we want to know if requests are failing.
			RetryIf: func(r *fasthttp.Request) bool { return false },
		},
	}
}

// Init prepares this tester to stress test the given URLs.
func (t *Tester) Init(urls []string) error {
	t.urls = urls
	t.responseByUrl = make(map[string]expectedResponseData)
	buffer := make([]byte, 0, bufferSize)
	log.Println("Expected response for URLs:")
	for _, u := range urls {
		code, body, err := t.client.Get(buffer, u)
		if err != nil {
			return fmt.Errorf("failed to fetch url %v: %v", u, err.Error())
		}
		bodyLen := len(body)
		t.responseByUrl[u] = expectedResponseData{
			StatusCode: code,
			MinLength:  bodyLen - bodyLengthAllowedChange,
			MaxLength:  bodyLen + bodyLengthAllowedChange,
		}
		log.Printf("%v | %v", code, u)
	}
	return nil
}

// Stress tests the urls in this tester by sending concurrent requests until the given context is
// canceled.
func (t *Tester) Stress(ctx context.Context, concurrency int) (*StressResult, error) {
	g, ctx := errgroup.WithContext(ctx)

	resultChan := make(chan StressResult)

	for i := 0; i < concurrency; i++ {
		g.Go(func() error {
			return t.fetchRandomUrls(ctx, resultChan)
		})
	}

	results := newStressResult()
	for i := 0; i < concurrency; i++ {
		r := <-resultChan
		results.merge(&r)
	}

	err := g.Wait()
	if err != nil {
		return nil, err
	}

	return results, nil
}

// String pretty-prints the key StressResult data as a table.
func (r *StressResult) String() string {
	b := strings.Builder{}
	lenLongestUrl := 0
	for u := range r.ResultsByUrl {
		uLen := len(u)
		if uLen > lenLongestUrl {
			lenLongestUrl = uLen
		}
	}
	urlHeading := "URL"
	successHeading := "Count Success"
	failureHeading := "Count Failure"
	latencyHeading := "Avg Latency (ms)"
	bytesHeading := "Avg Bytes / s"
	headerFormatString := fmt.Sprintf("%%-%ds | %%%ds | %%%ds | %%%ds | %%%ds\n", lenLongestUrl, len(successHeading), len(failureHeading), len(latencyHeading), len(bytesHeading))
	dataFormatString := fmt.Sprintf("%%-%ds | %%%dd | %%%dd | %%%d.3f | %%%d.3f\n", lenLongestUrl, len(successHeading), len(failureHeading), len(latencyHeading), len(bytesHeading))
	b.WriteString(fmt.Sprintf(headerFormatString, urlHeading, successHeading, failureHeading, latencyHeading, bytesHeading))
	b.WriteString(fmt.Sprintf(headerFormatString, strings.Repeat("-", lenLongestUrl), strings.Repeat("-", len(successHeading)), strings.Repeat("-", len(failureHeading)), strings.Repeat("-", len(latencyHeading)), strings.Repeat("-", len(bytesHeading))))
	urls := maps.Keys(r.ResultsByUrl)
	sort.Strings(urls)
	for _, u := range urls {
		ur := r.ResultsByUrl[u]
		numSucessfulCalls := ur.Successes.NumCalls
		if numSucessfulCalls == 0 {
			numSucessfulCalls = 1
		}
		successMillis := float64(ur.Successes.TotalLatency.Microseconds()) / 1000.0
		b.WriteString(fmt.Sprintf(dataFormatString, u, ur.Successes.NumCalls, ur.Failures.NumCalls, successMillis/float64(numSucessfulCalls), float64(ur.Successes.TotalBytesReceived)/successMillis))
	}
	return b.String()
}

func newStressResult() *StressResult {
	return &StressResult{
		ResultsByUrl: make(map[string]*ResultWithValidity),
	}
}

func (r *StressResult) add(url string, toAdd urlResult) {
	rv, isPresent := r.ResultsByUrl[url]
	if !isPresent {
		rv = &ResultWithValidity{}
		r.ResultsByUrl[url] = rv
	}
	if toAdd.isValid {
		rv.Successes.add(&toAdd)
	} else {
		rv.Failures.add(&toAdd)
	}
}

func (r *StressResult) merge(other *StressResult) {
	for u, orv := range other.ResultsByUrl {
		if rv, isPresent := r.ResultsByUrl[u]; isPresent {
			rv.merge(orv)
		} else {
			r.ResultsByUrl[u] = orv
		}
	}
}

func (rv *ResultWithValidity) merge(other *ResultWithValidity) {
	rv.Successes.merge(&other.Successes)
	rv.Failures.merge(&other.Failures)
}

func (r *AggregateResult) merge(other *AggregateResult) {
	r.NumCalls += other.NumCalls
	r.TotalBytesReceived += other.TotalBytesReceived
	r.TotalLatency += other.TotalLatency
}

func (r *AggregateResult) add(toAdd *urlResult) {
	r.NumCalls += 1
	r.TotalBytesReceived += int64(toAdd.bytesReceived)
	r.TotalLatency += toAdd.latency
}

func (exp *expectedResponseData) isValid(code int, body []byte) bool {
	bodyLen := len(body)
	return code == exp.StatusCode && bodyLen >= exp.MinLength && bodyLen <= exp.MaxLength
}

func (t *Tester) fetchRandomUrls(ctx context.Context, rc chan StressResult) error {
	buffer := make([]byte, 0, bufferSize)

	result := newStressResult()
	isDone := false
	for {
		select {
		case <-ctx.Done():
			isDone = true
		default:
			// Fall through
		}
		if isDone {
			break
		}

		u := t.randomURL()
		r, err := t.fetchAndVerifyUrl(buffer, u)
		if err != nil {
			rc <- StressResult{}
			return err
		}
		result.add(u, r)
	}

	rc <- *result
	return nil
}

func (t *Tester) randomURL() string {
	n := len(t.urls)
	i := rand.Int() % n
	return t.urls[i]
}

func (t *Tester) fetchAndVerifyUrl(buffer []byte, u string) (urlResult, error) {
	start := time.Now()
	code, body, err := t.client.Get(buffer, u)
	end := time.Now()
	if err != nil {
		return urlResult{}, err
	}

	exp := t.responseByUrl[u]
	return urlResult{
		isValid:       exp.isValid(code, body),
		bytesReceived: len(body),
		latency:       end.Sub(start),
	}, nil
}
