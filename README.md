# Shell Escape for Typst

This is a simple shell escape for [Typst](https://typst.app/). It allows you
to run shell commands directly from Typst compiler.

**That said, it does not mean that you _should_ run shell commands from Typst.**
In fact, I would highly recommend against it. This is a very dangerous feature
and should be used with extreme caution, and, if possible (it _is_ possible),
not at all.

## Usage

You don't.

## Usage

Please, don't. I beg you.

## Usage

Fine. But be aware that you aren't just playing with fire anymore. You are
planning with fire, in a forest, during a drought, with a flamethrower, near
the lake of gasoline, with fifteen nuclear power plants nearby.

Clone the repo and make sure you have
[cargo](https://www.rust-lang.org/tools/install) and
[Typst CLI](https://github.com/typst/typst) installed.

Run `cargo build`. This will create a binary in `/target`. Get the
`shell-escape.typ` file and `#import` it in your Typst project.
Run the built executable before compiling your project.

## A note of caution

> **This is a very dangerous feature. It's not just dangerous, it's _extremely_
> dangerous. There is a reason shell-escape will never be implemented in Typst
> Not only is it very bad for security, you can ruin your workspace. You are
> literally opening a window to an undefined behaviour from a safe environment
> of the Typst virtual machine.**
> 
> And don't even think of running this along with `typst-lsp`, or any other
> compiler instance. There will be no guarantees on the order of execution of
> commands. This _can_ result in the deadlock, and you will be lucky if only your
> compiler deadlocks.
> 
> **You have been warned.**

### High-level API

To run a command, use `#exec-command`.

| Argument                    | Type       | Description                                                                                                                         | Kind       | Default |
|-----------------------------|------------|-------------------------------------------------------------------------------------------------------------------------------------|------------|---------|
| `command`                   | `string`   | Command to run.                                                                                                                     | positional |         |
| `method-stdout`             | `function` | Function to call when the command writes to stdout, used to interpret stdout. For example, if command returns `.json`, pass `json`. | named      | `read`  |
| `method-stderr`             | `function` | Function to call when the command writes to stderr.                                                                                 | named      | `read`  |
| `format-stdout`             | `string`   | File extension of stdout. For example, if you want to read svg image, you should use `image` function with `".svg"` format          | named      | `""`    |
| `format-stderr`             | `string`   | File extension of stderr.                                                                                                           | named      | `""`    |
| `custom-hash`               | `string`   | Discriminator which helps defeat the limitation of function purity. Can be any string. If your command is pure, it's not needed.    | named      | `""`    |
| `allow-non-zero-error-code` | `bool`     | If `false`, the function will panic if command finishes execution with non-zero error code.                                         | named      | `true`  |

It returns a dictionary with three entries:

| Key          | Type                         | Description                                                |
|--------------|------------------------------|------------------------------------------------------------|
| `stdout`     | `any` (most likely `string`) | Stdout of the command, read with the given `method-stdout` |
| `stderr`     | `any` (most likely `string`) | Stderr of the command, read with the given `method-stdout` |
| `error-code` | `int`                        | Exit code of command                                       |

Example:

```typ
Calculate 2 + 2 using Python:
#exec-command("python -c \"print(2 + 2)\"")

Returns #(stdout: "4\n", stderr: "", error-code: 0)
```

See `example-*.typ` files for more.

### HTTP API (`curl` wrapper)

To make it easier to use, there is a wrapper around `curl` command for making
get-requests. It's called `#http-get`.

| Argument | Type       | Description                        | Kind       | Default |
|----------|------------|------------------------------------|------------|---------|
| `url`    | `string`   | URL to make a request to.          | positional |         |
| `method` | `function` | Function to interpret output with. | named      | `read`  |
| `format` | `string`   | File extension of the response.    | named      | `""`    |

There is also `#encode-url` function for URL parameter encoding.

### Low-level API

I will not document everything, but here is an overview:

- `#exec-command-async` queries a command for execution. It does not return
  anything.

- `#wait-one` waits for one command to finish execution. It returns a dictionary
  with two entries: `command` and `result`. There are no guarantees on the
  order of commands, so you need to check the `command` field to see which
  command finished execution.

- `#get-stdout` and `#get-stderr` return stdout and stderr of a last executed
  (and waited for) command respectively.

- `#reset-and-terminate-all` terminates all running commands. You should run it 
  before exiting your program.

In theory, this API allows you to run multiple commands in parallel, but I
wouldn't recommend it. It's not tested, just like everything else here, 
and I'm not sure if it works.

## How it works

It mounds a custom userspace filesystem. The only way Typst can interact with 
the outer world is by reading from files, and we are using this to our
advantage.

The filesystem is build in a way that allows us to build commands piece by 
piece and execute them. For example, you ran `#exec-command("ls -la /")`, Typst 
does the following (approximately):

```typ
Stop all running commands:
#read("<...>/reset")

Send hex-encoded command to the buffer:
#read("<...>/6c73202d6c61202f")

Request an execution of the command in the buffer:
#read("<...>/exec")

Wait for the command to finish execution:
#read("<...>/wait")

Check that command executed successfully:
#read("<...>/diagnostics")

Get the stdout of the command:
#read("<...>/stdout")

Get the stderr of the command:
#read("<...>/stderr")
```

Except, this won't quite work, because every function in Typst is cached,
so subsequent executions may not actually read the file. To fix this, we
need to add a "random" string at the start of every file path. This is what 
`discriminator` parameters are for. You should not care much about those,
unless you work with low-level API.

## Limitations

Linux only. Might work on other Unix-like systems or MacOS, but I haven't tested
it. Windows is not supported, do not ask.

Uses `fuse`. Make sure you have `user_allow_other` option enabled in
`/etc/fuse.conf`.

Currently, the filesystem is hardcoded to be mounted at
`/tmp/typst-shell-escape/shell-escape`. I probably should have made it
configurable, but I didn't. Change it in `main.rs`, and in `shell-escape.typ`
if you need to.

If the command you are running touches `/tmp/typst-shell-escape/shell-escape`
in any way, it will deadlock. This can be fixed, but I won't bother for now.
