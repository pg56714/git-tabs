; tree-sitter-yaml's query with two changes, so a file reads as it does on GitHub: plain values
; are left in the text colour (the original paints every plain scalar as a string), and keys are
; @tag (green) instead of @property.
(boolean_scalar) @boolean

(null_scalar) @constant.builtin

[
  (double_quote_scalar)
  (single_quote_scalar)
  (block_scalar)
] @string

[
  (integer_scalar)
  (float_scalar)
] @number

(comment) @comment

[
  (anchor_name)
  (alias_name)
] @label

(tag) @type

[
  (yaml_directive)
  (tag_directive)
  (reserved_directive)
] @attribute

(block_mapping_pair
  key: (flow_node
    [
      (double_quote_scalar)
      (single_quote_scalar)
    ] @tag))

(block_mapping_pair
  key: (flow_node
    (plain_scalar
      (string_scalar) @tag)))

(flow_mapping
  (_
    key: (flow_node
      [
        (double_quote_scalar)
        (single_quote_scalar)
      ] @tag)))

(flow_mapping
  (_
    key: (flow_node
      (plain_scalar
        (string_scalar) @tag))))

[
  ","
  "-"
  ":"
  ">"
  "?"
  "|"
] @punctuation.delimiter

[
  "["
  "]"
  "{"
  "}"
] @punctuation.bracket

[
  "*"
  "&"
  "---"
  "..."
] @punctuation.special
