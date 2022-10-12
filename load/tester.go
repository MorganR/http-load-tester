package load

import (
	"fmt"
	"log"

	"github.com/valyala/fasthttp"
)

const bodyLengthAllowedChange = 10

// Tester tests some URLs, performing basic validation as it goes.
type Tester struct {
	responseByUrl map[string]expectedResponseData
	client        *fasthttp.Client
}

type expectedResponseData struct {
	StatusCode int
	MinLength  int
	MaxLength  int
}

func NewTester() *Tester {
	return &Tester{
		client: &fasthttp.Client{
			Name: "http-load-tester",
			// Don't retry because we want to know if requests are failing.
			RetryIf: func(r *fasthttp.Request) bool { return false },
		},
	}
}

func (t *Tester) Init(urls []string) error {
	t.responseByUrl = make(map[string]expectedResponseData)
	buffer := make([]byte, 0, 16<<10)
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
