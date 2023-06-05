#import "shell-escape.typ": *

#raw(
    exec-command("ls -la .").stdout,
    block: true,
)
