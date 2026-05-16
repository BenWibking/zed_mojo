; Minimal Zed-safe Mojo highlights. Expand this incrementally after each query
; addition is verified against a real Zed launch.

(comment) @comment
(string) @string
(escape_sequence) @escape

[
  (integer)
  (float)
] @number

[
  (none)
  (true)
  (false)
] @constant.builtin

[
  "as"
  "async"
  "await"
  "break"
  "class"
  "comptime"
  "continue"
  "def"
  "elif"
  "else"
  "fn"
  "for"
  "from"
  "if"
  "import"
  "inout"
  "owned"
  "raises"
  "ref"
  "return"
  "struct"
  "trait"
  "try"
  "var"
  "while"
  "with"
] @keyword

[
  "-"
  "-="
  "!="
  "*"
  "**"
  "**="
  "*="
  "/"
  "//"
  "//="
  "/="
  "%"
  "%="
  "+"
  "+="
  "->"
  "<"
  "<="
  "="
  "=="
  ">"
  ">="
  "and"
  "in"
  "is"
  "not"
  "or"
] @operator
