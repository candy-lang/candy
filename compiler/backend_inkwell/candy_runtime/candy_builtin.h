#include "candy_runtime.h"

const candy_value_t *candy_builtin_equals(candy_value_t *left, candy_value_t *right, candy_value_t *responsible);
const candy_value_t *candy_builtin_if_else(candy_value_t *condition, candy_value_t *then, candy_value_t *otherwise, candy_value_t *responsible);
candy_value_t *candy_builtin_int_add(candy_value_t *left, candy_value_t *right, candy_value_t *responsible);
candy_value_t *candy_builtin_int_subtract(candy_value_t *left, candy_value_t *right, candy_value_t *responsible);
candy_value_t *candy_builtin_int_bit_length(candy_value_t *value, candy_value_t *responsible);
candy_value_t *candy_builtin_int_bitwise_and(candy_value_t *left, candy_value_t *right, candy_value_t *responsible);
candy_value_t *candy_builtin_int_bitwise_or(candy_value_t *left, candy_value_t *right, candy_value_t *responsible);
candy_value_t *candy_builtin_int_bitwise_xor(candy_value_t *left, candy_value_t *right, candy_value_t *responsible);
const candy_value_t *candy_builtin_int_compare_to(candy_value_t *left, candy_value_t *right, candy_value_t *responsible);
candy_value_t *candy_builtin_list_length(const candy_value_t *list, candy_value_t *responsible);
const candy_value_t *candy_builtin_print(candy_value_t *value, candy_value_t *responsible);
candy_value_t *candy_builtin_struct_get(candy_value_t *structure, candy_value_t *key, candy_value_t *responsible);
candy_value_t *candy_builtin_struct_get_keys(candy_value_t *structure, candy_value_t *responsible);
const candy_value_t *candy_builtin_tag_has_value(candy_value_t *tag, candy_value_t *responsible);
candy_value_t *candy_builtin_tag_get_value(candy_value_t *tag, candy_value_t *responsible);
candy_value_t *candy_builtin_tag_without_value(candy_value_t *tag, candy_value_t *responsible);
const candy_value_t *candy_builtin_struct_has_key(candy_value_t *structure, candy_value_t *key, candy_value_t *responsible);
const candy_value_t *candy_builtin_type_of(candy_value_t *value, candy_value_t *responsible);
