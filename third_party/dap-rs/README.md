# dap-rs, a Rust implementation of the Debug Adapter Protocol

## Introduction

This crate is a Rust implementation of the [Debug Adapter Protocol][1] (or DAP for short).

The best way to think of DAP is to compare it to [LSP][2] (Language Server Protocol) but
for debuggers. The core idea is the same: a protocol that serves as *lingua franca*
for editors and debuggers to talk to each other. This means that an editor that implements
DAP can use a debugger that also implements DAP.

In practice, the adapter might be separate from the actual debugger. For example, one could
implement an adapter that writes commands to the stdin of a gdb subprocess, then parses
the output it receives (this is why it's called an "adapter" - it adapts the debugger to
editors that know DAP).

## Stability

This crate is in a fairly early stage and breakages will be frequent. Any version before
1.0 might be a breaking version.

## Tutorial

For illustration purposes, we are going to recreate the `dummy-server` example, step by step.

To get started, create a binary project and add `dap` to your Cargo.toml:

```toml
[package]
name = "dummy-server"
version = "*"
edition = "2021"

[dependencies]
dap = "*"
```

Our dummy server is going to read its input from a text file and write the output to stdout.

To facilitate that, we import the necessary standard types.

Also, we are kinda lazy (err, smart) and we'll use `thiserror` to create a full-fledge error type.

```rust
use std::fs::File;
use std::io::{BufReader, BufWriter};

use thiserror::Error;
```

`dap` ships a `prelude` module and for most applications it's the easiest way to import the
necessary types:

```rust
use dap::prelude::*;
```

Let's get the error type out of the way first. We don't plan on handling all commands in our dummy
server, so we'll have an error variant that means that.

```rust
#[derive(Error, Debug)]
enum MyAdapterError {
  #[error("Unhandled command")]
  UnhandledCommandError,
}
```

Now we create our `Adapter` which is going to be the heart of the implementation.

Its `accept` function will be called for each incoming request, and each return type will be
returned to the client in its serialized form.

```rust
struct MyAdapter;

impl Adapter for MyAdapter {
  type Error = MyAdapterError;

  fn accept(&mut self, request: Request, _ctx: &mut dyn Context) -> Result<Response, Self::Error> {
    todo!()
  }
```

...whew. I probably could not explain that to my grandma. So what's with that return type?

Let's see:

  * The `Result` can be used to indicate a success or error in the Adapter itself. Since the
    `Server` does not know how to handle your custom errors, returning an error here will mean
    bubbling that error up through `Server::run`. In essence,
    this is an error for your application to handle.
  * If you want a user-visible indication of an error, you should return an error response. Users
    interact with their editor and it's the job of the editor to display such errors.
  * If, for any reason you don't want to send a response but you also don't want to return an error,
    you can use `Response::empty()`. But do note that clients will normally expect you to send a
    response, so use this sparingly.

The `request` argument is the deserialized request and its `command` field will be one of
the [requests][3] variants. In practice, this function will likely contain a large `match`
expression or some other means of dispatching the requests to code that can handle them.

The currently unused `_ctx` parameter could be used to send events and reverse requests to the
client. We are not going to utilize that in this tutorial (check out the `send_event` example).

We'll come back to implementing the `accept` function after we set up the infrastructure for
our server.

First, create an instance of your adapter in `main`:

```rust
let adapter = MyAdapter{};
```

Then, a client. In this crate, the `Client` is responsible for sending the responses, events and
reverse requests to the actual client that is connected.


```rust
let client = BasicClient::new(BufWriter::new(std::io::stdout()));
```

`BasicClient` is a builtin implementation that takes a `BufWriter` where the serialized
responses, event and reverse requests are written. It is easy and typical to write to the
standard output, but some implementations may want to write to a socket instead.

The `Client` and `Context` traits can be implemented to provide different behavior.

Next, we create the `Server`. The `Server` ties together the `Adapter` and the `Client`. Most
importantly, it is the server's responsibility to deserialize the incoming JSON requests,
pass them to the `Adapter`, then take the return value and pass it to the `Client` (which
in turn will serialize it and write it to the client's buffer - in this case, to stdout).


```rust
let mut server = Server::new(adapter, client);
```

Finally, we create a `BufReader` for the server which serves as the input mechanism and run the
server. In this example, we are reading the requests from a file, but in a real life implementation
this would be either stdin or a socket.

```rust
let f = File::open("testinput.txt")?;
let mut reader = BufReader::new(f);
```

And finally we run the server. It will run until EOF or an error from the adapter is encountered.

```rust
server.run(&mut reader)?;
```

The `AdapterError` variant of the error type that `run` may return will be the error type that you
defined above for your Adapter.

Let's take a look at the `accept` implementation now. To allow separating adapter output from
status messages, we will write the latter to stderr.

We will handle two commands in this example.

```rust
    match &request.command {
      Command::Initialize(args) => todo!(),
      Command::Next(_) => todo!(),
      _ => Err(MyAdapterError::UnhandledCommandError),
    }
  }
```

The `Next` command is one that only requires an ACK response. Keep in mind that it doesn't really
make sense to get a `Next` command from a client out of the blue, so this is purely for examples's
sake.

```rust
    match &request.command {
      Command::Initialize(args) => todo!(),
      Command::Next(_) => Ok(Response::make_ack(&request).unwrap()),
      _ => Err(MyAdapterError::UnhandledCommandError),
    }
  }
```

`Response`, `Event` and `ReverseRequest` have helper funtions to create them with certain defaults.

In this case, `make_ack` borrows the request to be able to copy the `seq` number from it. This
function returns a `Result` because it checks the request type and only creates an ACK for
requests that support it. We will ignore that potential error in this example and just
unwrap the result.

Let's implement the `Initialize` request now. We will make up an error where our adapter absolutely
needs the `client_name` (otherwise optional) field to be set. This is not really a sensible error,
we are doing it for demonstration purposes.

We also handle the happy path by returning a `Capabilites` response with some fields set. A
real-life application would like set many more fields. Overall, it looks like this:


```rust
    match &request.command {
      Command::Initialize(args) => {
        if let Some(client_name) = args.client_name.as_ref() {
          eprintln!("> Client '{client_name}' requested initialization.");
          Ok(Response::make_success(
            &request,
            ResponseBody::Initialize(Some(types::Capabilities {
              supports_configuration_done_request: Some(true),
              supports_evaluate_for_hovers: Some(true),
              ..Default::default()
            }),
          )))
        } else {
          Ok(Response::make_error(&request, "Missing client name"))
        }
      }
      Command::Next(_) => Ok(Response::make_ack(&request).unwrap()),
      _ => Err(MyAdapterError::UnhandledCommandError),
    }
  }
```

And that is it. The dummy server is ready to run now.

## License

This library is dual-licensed as MIT and Apache 2.0. That means users may choose either of these
licenses. In general, these are non-restrictive, non-viral licenses, a.k.a. *"do what you want
but no guarantees from me"*.

Commercial support is available on a contract basis (contact me: szelei.t@gmail.com).

[1]: https://microsoft.github.io/debug-adapter-protocol/
[2]: https://microsoft.github.io/language-server-protocol/
[3]: https://microsoft.github.io/debug-adapter-protocol/specification#Requests