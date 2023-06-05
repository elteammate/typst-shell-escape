#import "shell-escape.typ": *

#let python(code) = {
    if type(code) == "content" { code = code.text }
    code = code.replace("\\", "\\\\")
    code = code.replace("\"", "\\\"")
    exec-command("python -c \"" + code + "\"")
}

#python("print(\"Hello, world!\")")
#python("print(2 + 2)")

#let py(code) = python("print(" + code + ")").stdout

#py("2 + 2 * 2 + 2 * 2")

// If you uncomment the following line, you can enter a python expression
// in the terminal where you run the filesystem thingy.
// The compiler will wait, the filesystem should not.
// #py("eval(input())")
