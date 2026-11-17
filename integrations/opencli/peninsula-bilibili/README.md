# peninsula-bilibili

Read-only OpenCLI commands used by Peninsula:

```sh
opencli peninsula-bilibili dm -f json
opencli peninsula-bilibili likes -f json
```

The commands reuse OpenCLI's existing Chrome Browser Bridge session. They do
not accept or persist cookies and never call APIs that send messages, mark a
message read, or change a like. Bilibili may change these private web API
responses; the adapter reports an error when the expected response is absent.

Install this directory with `opencli plugin install file:///absolute/path/to/peninsula-bilibili`.
