; Don't format strings or comments
[
 (string_literal)
 (comment)
] @leaf


[
 "&"
 (comment)
 (else_clause)
 (elseif_clause)
 (subroutine)
 (subroutine_call)
 (derived_type_statement)
 (print_statement)
 (variable_declaration)
 (common_statement)
 (data_statement)
 (private_statement)
 (public_statement)
 (use_statement)
 (sequence_statement)
 ";"
] @append_hardline

(if_statement "if" @append_space)
(elseif_clause "elseif" @append_space)
(elseif_clause "elseif" @append_space)
(enumerator_statement "enumerator" @append_space)
(assignment "assignment" @append_space)


[
 ","
 (parenthesized_expression)
 (abstract_specifier)
 (procedure_qualifier)
 (block_label_start_expression)
 (intrinsic_type)
 (derived_type)
] @append_space

; start end blocks (hardline)
[
; start
(program_statement)
(module_statement)
(submodule_statement)
(subroutine_statement)
(interface_statement)
(function_statement)
(procedure_statement)
(generic_statement)
(final_statement)
(derived_type_definition)
(block_data_statement)
(enumeration_type_statement)
(assignment_statement)
(contains_statement)
(keyword_statement)
(enumerator_statement)
(do_loop (do_statement !do_label))
(if_statement)
; end
(end_program_statement)
(end_module_statement)
(end_submodule_statement)
(end_subroutine_statement)
(end_interface_statement)
(end_function_statement)
(end_type_statement)
(end_block_data_statement)
(end_enumeration_type_statement)
(implicit_statement)
(end_block_construct_statement)
] @append_hardline

; start end blocks (end space)
; TODO can we edit grammer.js to rename to end and _
; TODO this doesnt work as intended because tree-sitter currently combines end_
; into one node .e.g., it will keep "end program" separated but it wont
; separate "endprogram"
(end_program_statement "end" @append_space . "program")
(end_module_statement "end" @append_space . "module")
(end_submodule_statement "end" @append_space . "submodule")
(end_subroutine_statement "end" @append_space . "subroutine")
(end_interface_statement "end" @append_space . "interface")
(end_function_statement "end" @append_space . "function")
(end_type_statement "end" @append_space . "type")
(end_block_construct_statement "end" @append_space . "block")
(end_enumeration_type_statement "end" @append_space . "enumeration" @append_space)

("&" @prepend_space . "&")

(parenthesized_expression
  "(" @append_indent_start
  ")" @prepend_indent_end
)

(argument_list
  "(" @append_indent_start
  ")" @prepend_indent_end
)

(parameters
  "(" @append_indent_start
  ")" @prepend_indent_end
)

(included_items
  "only" ":" @append_indent_start
) @append_indent_end

(assignment_statement
    right: (math_expression) @prepend_indent_start
) @append_indent_end

; TODO: How to do flattened math expresssions?

(enumeration_type_statement "enumeration" @append_space . "type")

(implicit_statement _ @append_space)
(derived_type_statement _ @append_space)
(block_data_statement _ @append_space)
(common_statement _ @append_space)
(data_statement _ @append_space)
(interface_statement _ @append_space)
(use_statement _ @append_space)
(included_items _ @append_space)
; 'only: a, b'
(included_items "only" @append_antispace . ":")
(private_statement _ @append_space)
(public_statement _ @append_space)
(end_interface_statement _ @append_space)

(procedure_kind
    _ @append_space
(#not-eq? @append_space "(")
)

(subroutine_call "call" @append_space)

(block_construct "block" @append_hardline @append_indent_start)

[
 (program_statement)
 (module_statement)
 (submodule_statement)
 (subroutine_statement)
 (interface_statement)
 (function_statement)
 (derived_type_statement)
 (block_data_statement)
 (enumeration_type_statement)
] @append_indent_start

; A main program doesn't need a 'program' statement, only then is there an
; indentation block to close
(program (program_statement) (end_program_statement) @prepend_indent_end)

[
 (end_module_statement)
 (end_subroutine_statement)
 (end_interface_statement)
 (end_function_statement)
 (end_type_statement)
 (end_block_data_statement)
 (end_block_construct_statement)
 (end_submodule_statement)
 (end_enumeration_type_statement)
 (else_clause)
 (elseif_clause)
] @prepend_indent_end

[
 (function_result)
 (language_binding)
 (block_label)
] @prepend_space

[
 (assumed_size)
] @leaf

[
  (name)
  "::"
  "="
  "<"
  "<="
  "=="
  "=>"
  ">"
  "/="
  "+"
  "-"
  "/"
] @prepend_space @append_space

; Only a multiplication has spaces around "*", not e.g. 'a(*)', 'len=*' or
; 'print *'
(math_expression "*" @prepend_space @append_space)

[
 (format_identifier)
] @append_antispace

[
 (included_items)
] @prepend_antispace

(subroutine_statement "(" @prepend_antispace)
(subroutine_statement (parameters) @prepend_antispace)
(function_statement "(" @prepend_antispace)
(function_statement (parameters) @prepend_antispace)
(submodule_statement (module_name) @prepend_antispace @append_antispace)
("/" . (_) @prepend_antispace @append_antispace . "/")
; remove space around operators and assignment
(assignment "(" . _ @prepend_antispace @append_antispace . ")")
(operator "(" . _ @prepend_antispace @append_antispace . ")")

; make sure anything preceeding a comma "," has no space
((_) @append_antispace . ",")

; make sure anything preceeding a semicolon ";" has no space
((_) @append_antispace . ";")

(derived_type_statement
_ @append_antispace
.
","
)

; Statements not covered above end with a line break (the end of a statement
; is hidden in the parse tree)
[
 (allocate_statement)
 (arithmetic_if_statement)
 (assign_statement)
 (bind_statement)
 (close_statement)
 (coarray_statement)
 (deallocate_statement)
 (end_do_label_loop_statement do_label: (statement_label))
 (do_statement)
 (entry_statement)
 (equivalence_statement)
 (file_position_statement)
 (format_statement)
 (import_statement)
 (include_statement)
 (inquire_statement)
 (namelist_statement)
 (nullify_statement)
 (open_statement)
 (parameter_statement)
 (pointer_association_statement)
 (read_statement)
 (save_statement)
 (stop_statement)
 (write_statement)
 (if_statement)
 (do_loop (do_statement !do_label))
 (select_case_statement)
 (select_type_statement)
 (select_rank_statement)
 (where_statement)
 (forall_statement)
 (associate_statement)
 (block_construct)
 (end_if_statement)
 (end_do_loop_statement)
 (end_select_statement)
 (end_where_statement)
 (end_forall_statement)
 (end_associate_statement)
] @append_hardline

; A statement label is followed by its statement
(statement_label) @append_space
(do_label_virtual) @append_space

; Block constructs: the body is indented, the "end" statement is not
[
 (end_if_statement)
 (end_do_loop_statement)
 (end_where_statement)
 (end_forall_statement)
 (end_associate_statement)
 (elsewhere_clause)
] @prepend_indent_end

(end_if_statement "end" @append_space . "if")
(end_do_loop_statement "end" @append_space . "do")
(end_select_statement "end" @append_space . "select")
(end_where_statement "end" @append_space . "where")
(end_forall_statement "end" @append_space . "forall")
(end_associate_statement "end" @append_space . "associate")
(end_do_loop_statement "do" @append_space . (block_label))
(end_if_statement "if" @append_space . (block_label))
(end_select_statement "select" @append_space . (block_label))

(block_label_start_expression) @append_space

; if
(elseif_clause "else" @append_space . "if" @append_space)

; do: the header ends with the loop control, or with "do" alone
(do_statement !do_label "do" @append_space)
(while_statement "while" @append_space)
(do_statement !do_label
  [
   (loop_control_expression)
   (while_statement)
   (concurrent_statement)
  ] @append_hardline @append_indent_start
)
(do_loop
  (do_statement !do_label "do" @append_hardline @append_indent_start .)
  .
  [
   (comment)
   (statement_label)
   (end_do_loop_statement)
   (assignment_statement)
   (pointer_association_statement)
   (subroutine_call)
   (keyword_statement)
   (if_statement)
   (arithmetic_if_statement)
   (where_statement)
   (forall_statement)
   (select_case_statement)
   (select_type_statement)
   (select_rank_statement)
   (do_loop)
   (end_do_label_loop_statement)
   (format_statement)
   (open_statement)
   (close_statement)
   (print_statement)
   (write_statement)
   (read_statement)
   (inquire_statement)
   (stop_statement)
   (block_construct)
   (associate_statement)
   (file_position_statement)
   (allocate_statement)
   (deallocate_statement)
   (nullify_statement)
   (entry_statement)
   (assign_statement)
   (coarray_statement)
   (coarray_team_statement)
   (coarray_critical_statement)
   (call_expression)
   (include_statement)
  ]
)

; select case/type/rank: the cases are at the level of "select", their bodies
; are indented
(select_case_statement "select" @append_space . "case" @append_space)
(select_type_statement "select" @append_space . "type" @append_space)
(select_rank_statement "select" @append_space . "rank" @append_space)
[
 (select_case_statement (selector) @append_hardline)
 (select_type_statement (selector) @append_hardline)
 (select_rank_statement (selector) @append_hardline)
]
[
 (case_statement)
 (type_statement)
 (rank_statement)
] @append_indent_end @append_hardline
(case_statement "case" @append_space)
(type_statement ["type" "class"] @append_space . "is" @append_space)
(type_statement "class" @append_space . (default))
(type_statement "(" . (_) @prepend_antispace @append_antispace . ")")
(rank_statement "rank" @append_space)

; where: only the block form has a body on separate lines
(where_statement "where" @append_space)
(where_statement
  (parenthesized_expression) @append_hardline @append_indent_start
  (end_where_statement)
)

; forall
(forall_statement "forall" @append_space)
(forall_statement ")" @append_space)
(forall_statement
  ")" @append_hardline @append_indent_start
  (end_forall_statement)
)

; associate
(associate_statement "associate" @append_space)
(associate_statement (association_list ")" @append_hardline @append_indent_start))

; Input/output and other statements with a parenthesised list
[
 (write_statement "(" . (_) @prepend_antispace)
 (read_statement "(" . (_) @prepend_antispace)
 (open_statement "(" . (_) @prepend_antispace)
 (close_statement "(" . (_) @prepend_antispace)
 (inquire_statement "(" . (_) @prepend_antispace)
 (allocate_statement "(" . (_) @prepend_antispace)
 (deallocate_statement "(" . (_) @prepend_antispace)
 (nullify_statement "(" . (_) @prepend_antispace)
 (format_statement "(" . (_) @prepend_antispace)
]
(write_statement ")" @append_space . (output_item_list))
(read_statement ")" @append_space . (input_item_list))
(read_statement "read" @append_space . (format_identifier))
(print_statement "print" @append_space)
(file_position_statement . _ @append_space)
(stop_statement ["error" "stop"] @append_space)
; goto, cycle, exit, ... followed by a label
(keyword_statement _ @append_space . (_))

; Logical and relational operators (also '.and.', '.eq.', ...) are surrounded
; by spaces, e.g. '0 .and. x' must not become '0.and.x'
(logical_expression operator: _ @prepend_space @append_space)
(relational_expression operator: _ @prepend_space @append_space)
; '.not.'
(logical_expression . _ @append_space . (_) .)

; Attribute statements: 'external f, g', 'intent(in) :: a', 'save s', ...
[
 (variable_modification)
 (cray_pointer_declaration)
] @append_hardline
(variable_modification (type_qualifier) @append_space)
(save_statement "save" @append_space)
(parameter_statement "parameter" @append_space)

; Numbers with a kind ('3.14_dp') have nodes only for the kind, the digits
; would be lost
(number_literal) @leaf

; enum, bind(c) ... end enum
(enum_statement) @append_hardline @append_indent_start
(enum_statement "enum" @append_antispace . ",")
(end_enum_statement) @prepend_indent_end @append_hardline
(end_enum_statement "end" @append_space . "enum")

; Separate module procedures in submodules
(module_procedure_statement) @append_hardline @append_indent_start
(module_procedure_statement "module" @append_space . "procedure" @append_space)
(end_module_procedure_statement) @prepend_indent_end @append_hardline
(end_module_procedure_statement "end" @append_space . "procedure")
(end_module_procedure_statement "procedure" @append_space . (name))

; 'end block data' is not parsed correctly, keep it as it is
(end_block_data_statement) @leaf
(block_data) @append_hardline

; critical ... end critical (and 'change team' statements)
(coarray_critical_statement "critical" @append_hardline @append_indent_start)
(end_coarray_critical_statement) @prepend_indent_end
(end_coarray_critical_statement "end" @append_space . "critical")
[
 (coarray_critical_statement)
 (end_coarray_critical_statement)
 (coarray_team_statement)
 (end_coarray_team_statement)
] @append_hardline
(end_coarray_team_statement "end" @append_space . "team")

; sync all, sync images (...), lock (...), ...
(coarray_statement . _ @append_space . _)

; 'use, intrinsic :: ...'
(use_statement "use" @append_antispace . ",")
; 'implicit none (type, external)'
(implicit_statement "(" @prepend_space @append_antispace)
(implicit_statement _ @append_antispace . [")" ","])

; The header of a block ends with a line break, after the block label if there
; is one ('case (1) name', 'else name'). The body is indented. (Generated for
; each construct: a query can't express 'not followed by a block label'.)
(if_statement
  "then" @append_hardline @append_indent_start
  .
  [(comment) (statement_label) (ERROR)
   (assignment_statement) (pointer_association_statement) (subroutine_call)
   (keyword_statement) (if_statement) (arithmetic_if_statement)
   (where_statement) (forall_statement) (select_case_statement)
   (select_type_statement) (select_rank_statement) (do_loop (do_statement !do_label))
   (do_statement) (end_do_label_loop_statement do_label: (statement_label)) (format_statement)
   (open_statement) (close_statement) (print_statement) (write_statement)
   (read_statement) (inquire_statement) (stop_statement) (block_construct)
   (associate_statement) (file_position_statement) (allocate_statement)
   (deallocate_statement) (nullify_statement) (entry_statement)
   (assign_statement) (coarray_statement) (coarray_team_statement)
   (coarray_critical_statement) (call_expression) (include_statement)
   (elseif_clause) (else_clause) (elsewhere_clause) (end_if_statement)
   (end_where_statement) (end_coarray_team_statement)]
)
(if_statement "then" @append_hardline @append_indent_start .)
(if_statement "then" @append_space . [(block_label) (parenthesized_expression)])
(if_statement (block_label) @append_hardline @append_indent_start)
(elseif_clause
  "then" @append_hardline @append_indent_start
  .
  [(comment) (statement_label) (ERROR)
   (assignment_statement) (pointer_association_statement) (subroutine_call)
   (keyword_statement) (if_statement) (arithmetic_if_statement)
   (where_statement) (forall_statement) (select_case_statement)
   (select_type_statement) (select_rank_statement) (do_loop (do_statement !do_label))
   (do_statement) (end_do_label_loop_statement do_label: (statement_label)) (format_statement)
   (open_statement) (close_statement) (print_statement) (write_statement)
   (read_statement) (inquire_statement) (stop_statement) (block_construct)
   (associate_statement) (file_position_statement) (allocate_statement)
   (deallocate_statement) (nullify_statement) (entry_statement)
   (assign_statement) (coarray_statement) (coarray_team_statement)
   (coarray_critical_statement) (call_expression) (include_statement)
   (elseif_clause) (else_clause) (elsewhere_clause) (end_if_statement)
   (end_where_statement) (end_coarray_team_statement)]
)
(elseif_clause "then" @append_hardline @append_indent_start .)
(elseif_clause "then" @append_space . [(block_label) (parenthesized_expression)])
(elseif_clause (block_label) @append_hardline @append_indent_start)
(else_clause
  "else" @append_hardline @append_indent_start
  .
  [(comment) (statement_label) (ERROR)
   (assignment_statement) (pointer_association_statement) (subroutine_call)
   (keyword_statement) (if_statement) (arithmetic_if_statement)
   (where_statement) (forall_statement) (select_case_statement)
   (select_type_statement) (select_rank_statement) (do_loop (do_statement !do_label))
   (do_statement) (end_do_label_loop_statement do_label: (statement_label)) (format_statement)
   (open_statement) (close_statement) (print_statement) (write_statement)
   (read_statement) (inquire_statement) (stop_statement) (block_construct)
   (associate_statement) (file_position_statement) (allocate_statement)
   (deallocate_statement) (nullify_statement) (entry_statement)
   (assign_statement) (coarray_statement) (coarray_team_statement)
   (coarray_critical_statement) (call_expression) (include_statement)
   (elseif_clause) (else_clause) (elsewhere_clause) (end_if_statement)
   (end_where_statement) (end_coarray_team_statement)]
)
(else_clause "else" @append_hardline @append_indent_start .)
(else_clause "else" @append_space . [(block_label) (parenthesized_expression)])
(else_clause (block_label) @append_hardline @append_indent_start)
(case_statement
  [")" (default)] @append_hardline @append_indent_start
  .
  [(comment) (statement_label) (ERROR)
   (assignment_statement) (pointer_association_statement) (subroutine_call)
   (keyword_statement) (if_statement) (arithmetic_if_statement)
   (where_statement) (forall_statement) (select_case_statement)
   (select_type_statement) (select_rank_statement) (do_loop (do_statement !do_label))
   (do_statement) (end_do_label_loop_statement do_label: (statement_label)) (format_statement)
   (open_statement) (close_statement) (print_statement) (write_statement)
   (read_statement) (inquire_statement) (stop_statement) (block_construct)
   (associate_statement) (file_position_statement) (allocate_statement)
   (deallocate_statement) (nullify_statement) (entry_statement)
   (assign_statement) (coarray_statement) (coarray_team_statement)
   (coarray_critical_statement) (call_expression) (include_statement)
   (elseif_clause) (else_clause) (elsewhere_clause) (end_if_statement)
   (end_where_statement) (end_coarray_team_statement)]
)
(case_statement [")" (default)] @append_hardline @append_indent_start .)
(case_statement [")" (default)] @append_space . [(block_label) (parenthesized_expression)])
(case_statement (block_label) @append_hardline @append_indent_start)
(type_statement
  [")" (default)] @append_hardline @append_indent_start
  .
  [(comment) (statement_label) (ERROR)
   (assignment_statement) (pointer_association_statement) (subroutine_call)
   (keyword_statement) (if_statement) (arithmetic_if_statement)
   (where_statement) (forall_statement) (select_case_statement)
   (select_type_statement) (select_rank_statement) (do_loop (do_statement !do_label))
   (do_statement) (end_do_label_loop_statement do_label: (statement_label)) (format_statement)
   (open_statement) (close_statement) (print_statement) (write_statement)
   (read_statement) (inquire_statement) (stop_statement) (block_construct)
   (associate_statement) (file_position_statement) (allocate_statement)
   (deallocate_statement) (nullify_statement) (entry_statement)
   (assign_statement) (coarray_statement) (coarray_team_statement)
   (coarray_critical_statement) (call_expression) (include_statement)
   (elseif_clause) (else_clause) (elsewhere_clause) (end_if_statement)
   (end_where_statement) (end_coarray_team_statement)]
)
(type_statement [")" (default)] @append_hardline @append_indent_start .)
(type_statement [")" (default)] @append_space . [(block_label) (parenthesized_expression)])
(type_statement (block_label) @append_hardline @append_indent_start)
(rank_statement
  [")" (default)] @append_hardline @append_indent_start
  .
  [(comment) (statement_label) (ERROR)
   (assignment_statement) (pointer_association_statement) (subroutine_call)
   (keyword_statement) (if_statement) (arithmetic_if_statement)
   (where_statement) (forall_statement) (select_case_statement)
   (select_type_statement) (select_rank_statement) (do_loop (do_statement !do_label))
   (do_statement) (end_do_label_loop_statement do_label: (statement_label)) (format_statement)
   (open_statement) (close_statement) (print_statement) (write_statement)
   (read_statement) (inquire_statement) (stop_statement) (block_construct)
   (associate_statement) (file_position_statement) (allocate_statement)
   (deallocate_statement) (nullify_statement) (entry_statement)
   (assign_statement) (coarray_statement) (coarray_team_statement)
   (coarray_critical_statement) (call_expression) (include_statement)
   (elseif_clause) (else_clause) (elsewhere_clause) (end_if_statement)
   (end_where_statement) (end_coarray_team_statement)]
)
(rank_statement [")" (default)] @append_hardline @append_indent_start .)
(rank_statement [")" (default)] @append_space . [(block_label) (parenthesized_expression)])
(rank_statement (block_label) @append_hardline @append_indent_start)
(elsewhere_clause
  ["elsewhere" "where" (parenthesized_expression)] @append_hardline @append_indent_start
  .
  [(comment) (statement_label) (ERROR)
   (assignment_statement) (pointer_association_statement) (subroutine_call)
   (keyword_statement) (if_statement) (arithmetic_if_statement)
   (where_statement) (forall_statement) (select_case_statement)
   (select_type_statement) (select_rank_statement) (do_loop (do_statement !do_label))
   (do_statement) (end_do_label_loop_statement do_label: (statement_label)) (format_statement)
   (open_statement) (close_statement) (print_statement) (write_statement)
   (read_statement) (inquire_statement) (stop_statement) (block_construct)
   (associate_statement) (file_position_statement) (allocate_statement)
   (deallocate_statement) (nullify_statement) (entry_statement)
   (assign_statement) (coarray_statement) (coarray_team_statement)
   (coarray_critical_statement) (call_expression) (include_statement)
   (elseif_clause) (else_clause) (elsewhere_clause) (end_if_statement)
   (end_where_statement) (end_coarray_team_statement)]
)
(elsewhere_clause ["elsewhere" "where" (parenthesized_expression)] @append_hardline @append_indent_start .)
(elsewhere_clause ["elsewhere" "where" (parenthesized_expression)] @append_space . [(block_label) (parenthesized_expression)])
(elsewhere_clause (block_label) @append_hardline @append_indent_start)
(coarray_team_statement
  (argument_list) @append_hardline @append_indent_start
  .
  [(comment) (statement_label) (ERROR)
   (assignment_statement) (pointer_association_statement) (subroutine_call)
   (keyword_statement) (if_statement) (arithmetic_if_statement)
   (where_statement) (forall_statement) (select_case_statement)
   (select_type_statement) (select_rank_statement) (do_loop (do_statement !do_label))
   (do_statement) (end_do_label_loop_statement do_label: (statement_label)) (format_statement)
   (open_statement) (close_statement) (print_statement) (write_statement)
   (read_statement) (inquire_statement) (stop_statement) (block_construct)
   (associate_statement) (file_position_statement) (allocate_statement)
   (deallocate_statement) (nullify_statement) (entry_statement)
   (assign_statement) (coarray_statement) (coarray_team_statement)
   (coarray_critical_statement) (call_expression) (include_statement)
   (elseif_clause) (else_clause) (elsewhere_clause) (end_if_statement)
   (end_where_statement) (end_coarray_team_statement)]
)
(coarray_team_statement (argument_list) @append_hardline @append_indent_start .)

; Labelled do loops: 'do 10 i = 1, n' ... '10 continue' (not indented)
(do_statement "do" @append_space (statement_label_reference))
(do_statement (statement_label_reference) @append_space)
(end_do_label_loop_statement "end" @append_space . "do")

; change team (...) ... end team
(coarray_team_statement "change" @append_space . "team" @append_space)
(end_coarray_team_statement) @prepend_indent_end
; 'rank (*)' isn't parsed and may swallow the 'rank' of a following 'rank default'
(rank_statement (default) @prepend_space)

; Keep blank lines between statements (at most one in a row)
(_) @allow_blank_line_before

; 'procedure(iface) f' (declaration without '::')
(variable_declaration (procedure) @append_space)

; Keywords that may be written as two words
(elsewhere_clause "else" @append_space . "where")
(intrinsic_type "double" @append_space)
(keyword_statement "go" @append_space . "to")

; 'character c*4': the length has no node of its own
(character_length) @leaf

; A parse error ending a line (e.g. invalid code) is not merged with the next
; line (lines consisting only of errors are kept as they are, see errors.rs)
(ERROR) @append_input_softline
; computed 'go to (10, 20) i'
(keyword_statement "to" @append_space . "(")
(keyword_statement "(" @append_antispace)
; 'intent(in out)'
(type_qualifier "in" @append_space . "out")

; 'contains' closes the block of the program unit and opens a new one, if the
; program unit has an opening statement (a main program may not have one)
[
 (module (internal_procedures (contains_statement "contains" @prepend_indent_end @append_indent_start)))
 (submodule (internal_procedures (contains_statement "contains" @prepend_indent_end @append_indent_start)))
 (subroutine (internal_procedures (contains_statement "contains" @prepend_indent_end @append_indent_start)))
 (function (internal_procedures (contains_statement "contains" @prepend_indent_end @append_indent_start)))
 (module_procedure (internal_procedures (contains_statement "contains" @prepend_indent_end @append_indent_start)))
 (program (program_statement) (internal_procedures (contains_statement "contains" @prepend_indent_end @append_indent_start)))
 (derived_type_definition (derived_type_procedures (contains_statement "contains" @prepend_indent_end @append_indent_start)))
]
