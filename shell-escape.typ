#let hex-digits = "0123456789abcdef"

#let hex(x) = {
  hex-digits.at(int(x / 16))
  hex-digits.at(calc.rem(x, 16))
}

#let ascii-table = {
  let result = (:)

  for (i, c) in range(128)
      .map(c => eval("[\u{" + hex(c) + "}]").text)
      .enumerate() {
    result.insert(c, i)
  }

  result
}

#let encode-int(x) = {
  let alpha = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
  let base = alpha.len()

  while x > 0 {
    alpha.at(calc.rem(x, base))
    x = int(x / base)
  }
}

#let hash(obj) = {
  obj = repr(obj)
  let (a1, a2, a3, a4, h1, h2, h3, h4, mod1, mod2, mod3, mod4) = (
    911, 1642, 7256, 5134, 60298, 134587, 18096, 109863,
    1000000007, 1300000721, 1500004447, 1800003419
  )

  for c in obj.clusters() {
    let utf-8 = ascii-table.at(c)
    h1 = calc.rem(h1 * a1 + utf-8, mod1)
    h2 = calc.rem(h2 * a2 + utf-8, mod2)
    h3 = calc.rem(h3 * a3 + utf-8, mod3)
    h4 = calc.rem(h4 * a4 + utf-8, mod4)
  }

  encode-int(h1)
  encode-int(h2)
  encode-int(h3)
  encode-int(h4)
}

#let encode-hex(s) = {
  for c in s.clusters() {
    hex(ascii-table.at(c))
  }
}

#let encode-url(s) = {
  let representable = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_~"
  let result = ""
  for c in s.clusters() {
    if representable.contains(c) {
      result += c
    } else {
      result += "%" + hex(ascii-table.at(c))
    }
  }
  result
}

#let shell-escape-root = "//tmp/typst-shell-escape/shell-escape/"

#let do-with-shell-escape(action, hash, fn: read) = {
  let path = shell-escape-root + hash + "_" + action
  fn(path)
}

#let chunks(s, n) = {
  let result = ()
  for (i, c) in s.clusters().enumerate() {
    if calc.rem(i, n) == 0 {
      result.push(c)
    } else {
      result.last() += c
    }
  }
  result
}

#let reset-and-terminate-all(discriminator: "") = {
  assert.eq("!", do-with-shell-escape("reset", discriminator))
}

#let exec-command-async(
  command,
  discriminator: "",
) = {
  let disc-hash = hash(discriminator + "gIbBeRiSh" + command)
  reset-and-terminate-all(discriminator: disc-hash)
  for part in chunks(command, 32) {
    let part-hash = hash(encode-hex(part) + disc-hash)
    assert.eq("!", do-with-shell-escape(encode-hex(part), part-hash))
  }
  assert.eq("!", do-with-shell-escape("exec", disc-hash))
}

#let wait-one(
  discriminator: "",
  allow-non-zero-error-code: true,
) = {
  let disc-hash = hash(discriminator + "gIbBeRiSh")
  assert.eq("!", do-with-shell-escape("wait", disc-hash))
  do-with-shell-escape("diagnostics", disc-hash, fn: json)
}

#let get-stdout(discriminator: "", method: read, format: "") = {
  let disc-hash = hash(discriminator + "gIbBeRiSh")
  do-with-shell-escape("stdout" + format, disc-hash, fn: method)
}

#let get-stderr(discriminator: "", method: read, format: "") = {
  let disc-hash = hash(discriminator + "gIbBeRiSh")
  do-with-shell-escape("stderr" + format, disc-hash, fn: method)
}

#let exec-command(
  command,
  method-stdout: read, 
  method-stderr: read,
  format-stdout: "",
  format-stderr: "",
  custom-hash: "",
  allow-non-zero-error-code: true,
) = {
  let command-hash = hash(command + "GiBbErIsH" + custom-hash)
  exec-command-async(command, discriminator: command-hash)
  let data = wait-one(discriminator: command-hash)

  if data.command.trim() != command.trim() {
    panic("Executed command mismatches with the one requested: " + data.command + " != " + command)
  }

  if not data.result.ran {
    panic("Failed to execute command: ", data.result.error)
  }

  if not allow-non-zero-error-code {
    assert.eq(data.result.error_code, 0, message: "Exit code is not zero")
  }

  let stdout = get-stdout(discriminator: command-hash, method: method-stdout, format: format-stdout)
  let stderr = get-stderr(discriminator: command-hash, method: method-stderr, format: format-stderr)

  (stdout: stdout, stderr: stderr, error-code: data.result.error_code)
}

#let http-get(url, method: read, format: "") = {
  let command = "curl -sS \"" + url + "\""
  let result = exec-command(command, method-stdout: method, format-stdout: format)
  if result.error-code != 0 {
    panic("Failed to execute command: ", result.stderr)
  }
  result.stdout
}

/*
// #exec-command("ls -la /")
// #exec-command("sleep 1")
#set page(paper: "a8")
$
2 + 2 dot 2 = #exec-command("python -c \"print(2 + 2 * 2)\"").stdout
$

#http-get(
  "https://latex.codecogs.com/svg.image?%5Cfrac%7B4%7D%7B5%7D&plus;%5Cpi%5COmega%5Cint_%7B2%5Cpi%7D%5E%7B%5Cinfty%7D%7B5%5Cleft%5C(%5Cfrac%7B%5Ctau&plus;3%7D%7B2%7D%5Cright%5C)d%5Comega%7D)",
  method: image,
  format: ".svg",
)
*/
