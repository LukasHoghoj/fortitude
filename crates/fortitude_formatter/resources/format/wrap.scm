; Lines are not broken within these nodes
(type_qualifier) @no-break
; nor within the left-hand side of an assignment. '@no-split' nodes aren't broken
; at blanks within or next to them either, even if the line remains too long
(assignment_statement left: (_) @no-break @no-split)
(pointer_association_statement . (_) @no-break @no-split)
; nor within the condition of a one-line if statement: one whose condition is
; followed by a single statement, and no 'end if'. Nor is it broken right
; after the condition, which is misleading (C152)
(if_statement
  (parenthesized_expression) @no-break @no-split
  .
  [(assignment_statement) (pointer_association_statement) (subroutine_call)
   (keyword_statement) (arithmetic_if_statement) (where_statement)
   (forall_statement) (open_statement) (close_statement) (print_statement)
   (write_statement) (read_statement) (inquire_statement) (stop_statement)
   (file_position_statement) (allocate_statement) (deallocate_statement)
   (nullify_statement) (assign_statement) (coarray_statement)
   (call_expression)]
  .
)

; Move the right-hand side to the next line if it then fits, rather than
; breaking within it
(assignment_statement "=" @break-to-fit) @root
(pointer_association_statement "=>" @break-to-fit) @root

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
