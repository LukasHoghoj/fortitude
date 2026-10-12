; Lines are not broken within these nodes
(type_qualifier) @no-break

(included_items 
[
  ("only" . ":" @break-after)
  ((_) "," @break-after)
]
) @root

(argument_list
    ["(" ","] @break-after

) @root

(parameters
    ["(" ","] @break-after
) @root

(variable_declaration
  "::" "," @break-after
) @root

(math_expression
  left: (_) @break-after
  right: (_)
) @root

(output_item_list
    "," @break-after
) @root

(logical_expression
  left: (_) @break-after
  right: (_)
) @root

(concatenation_expression
  "//" @break-after
) @root

(array_literal
  ["[" "(/" ","] @break-after
) @root

(derived_type_member_expression
  "%" @break-after
) @root

(allocate_statement
  ["(" ","] @break-after
) @root

(deallocate_statement
  ["(" ","] @break-after
) @root

(nullify_statement
  ["(" ","] @break-after
) @root

(open_statement
  ["(" ","] @break-after
) @root

(close_statement
  ["(" ","] @break-after
) @root

(inquire_statement
  ["(" ","] @break-after
) @root

(size
  ["(" ","] @break-after
) @root
