# Rate Limit

DeepSeek API dynamically limits user concurrency based on server load. When you reach the concurrency limit, you will immediately receive an HTTP 429 response.

After a request is sent, it may take some time to receive a response. During this period, the HTTP connection remains open and you may continuously receive:

- **Non-streaming**: Empty lines
- **Streaming**: SSE keep-alive comments (`: keep-alive`)

These do not affect JSON body parsing. If you parse HTTP responses yourself, handle these empty lines or comments appropriately.

If the request has not started inference after 10 minutes, the server will close the connection.
