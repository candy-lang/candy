#include "candy_rt.h"

candy_value_t *candy_builtin_equals(candy_value_t *left, candy_value_t *right);
candy_value_t *candy_builtin_ifelse(candy_value_t *condition, candy_value_t *then, candy_value_t *otherwise);
candy_value_t *candy_builtin_int_add(candy_value_t *left, candy_value_t *right);
candy_value_t *candy_builtin_int_subtract(candy_value_t *left, candy_value_t *right);
candy_value_t *candy_builtin_int_bit_length(candy_value_t *value);
candy_value_t *candy_builtin_int_bitwise_and(candy_value_t *left, candy_value_t *right);
candy_value_t *candy_builtin_int_bitwise_or(candy_value_t *left, candy_value_t *right);
candy_value_t *candy_builtin_int_bitwise_xor(candy_value_t *left, candy_value_t *right);
candy_value_t *candy_builtin_int_compareto(candy_value_t *left, candy_value_t *right);
candy_value_t *candy_builtin_list_length(candy_value_t *list);
candy_value_t *candy_builtin_struct_get(candy_value_t *structure, candy_value_t *key);
candy_value_t *candy_builtin_struct_get_keys(candy_value_t *structure);
candy_value_t *candy_builtin_struct_has_key(candy_value_t *structure, candy_value_t *key);
candy_value_t *candy_builtin_typeof(candy_value_t *value);