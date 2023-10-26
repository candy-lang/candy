#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include "candy_runtime.h"

const candy_value_t *candy_builtin_equals(candy_value_t *left, candy_value_t *right, candy_value_t *responsible)
{
    if (left
            ->type != right->type)
    {
        return &__internal_false;
    }
    switch (left->type)
    {
    case CANDY_TYPE_INT:
        return to_candy_bool(left->value.integer == right->value.integer);
        break;
    case CANDY_TYPE_TAG:
        return to_candy_bool(strcmp(left->value.text, right->value.text) == 0);
    default:
        return &__internal_false;
    }
}

const candy_value_t *candy_builtin_if_else(candy_value_t *condition, candy_value_t *then, candy_value_t *otherwise, candy_value_t *responsible)
{
    candy_value_t *body = candy_tag_to_bool(condition) ? then : otherwise;
    candy_function function = (body->value).function.function;
    candy_value_t *environment = (body->value).function.environment;
    return function(environment);
}

candy_value_t *candy_builtin_int_add(candy_value_t *left, candy_value_t *right, candy_value_t *responsible)
{
    return make_candy_int(left->value.integer + right->value.integer);
}

candy_value_t *candy_builtin_int_subtract(candy_value_t *left, candy_value_t *right, candy_value_t *responsible)
{
    return make_candy_int(left->value.integer - right->value.integer);
}

candy_value_t *candy_builtin_int_bit_length(candy_value_t *value, candy_value_t *responsible)
{
    int64_t int_value = value->value.integer;
    int is_negative = int_value < 0;
    if (is_negative)
    {
        int_value = -int_value;
    }
    int shifts = 0;
    while (int_value)
    {
        int_value = int_value >> shifts;
        shifts++;
    }
    return make_candy_int(shifts + is_negative);
}

candy_value_t *candy_builtin_int_bitwise_and(candy_value_t *left, candy_value_t *right, candy_value_t *responsible)
{
    return make_candy_int(left->value.integer & right->value.integer);
}

candy_value_t *candy_builtin_int_bitwise_or(candy_value_t *left, candy_value_t *right, candy_value_t *responsible)
{
    return make_candy_int(left->value.integer | right->value.integer);
}

candy_value_t *candy_builtin_int_bitwise_xor(candy_value_t *left, candy_value_t *right, candy_value_t *responsible)
{
    return make_candy_int(left->value.integer ^ right->value.integer);
}

const candy_value_t *candy_builtin_int_compare_to(candy_value_t *left, candy_value_t *right, candy_value_t *responsible)
{
    int64_t left_value = left->value.integer;
    int64_t right_value = right->value.integer;
    if (left_value < right_value)
    {
        return &__internal_less;
    }
    else if (left_value == right_value)
    {
        return &__internal_equal;
    }
    else
    {
        return &__internal_greater;
    }
}

candy_value_t *candy_builtin_list_length(const candy_value_t *list, candy_value_t *responsible)
{
    size_t index = 0;
    while (list->value.list[index] != NULL)
    {
        index++;
    }
    return make_candy_int(index);
}

const candy_value_t *candy_builtin_print(candy_value_t *value, candy_value_t *responsible)
{
    print_candy_value(value);
    printf("\n");
    return &__internal_nothing;
}

candy_value_t *candy_builtin_struct_get(candy_value_t *structure, candy_value_t *key, candy_value_t *responsible)
{
    size_t index = 0;
    while (!candy_tag_to_bool(candy_builtin_equals(structure->value.structure.keys[index], key, responsible)))
    {
        index++;
    }
    return structure->value.structure.values[index];
}

candy_value_t *candy_builtin_struct_get_keys(candy_value_t *structure, candy_value_t *responsible)
{
    return make_candy_list(structure->value.structure.keys);
}

const candy_value_t *candy_builtin_struct_has_key(candy_value_t *structure, candy_value_t *key, candy_value_t *responsible)
{
    size_t index = 0;
    while (structure->value.structure.keys[index] != NULL)
    {
        if (candy_tag_to_bool(candy_builtin_equals(structure->value.structure.keys[index], key, responsible)))
        {
            return &__internal_true;
        }
    }
    return &__internal_false;
}

const candy_value_t *candy_builtin_tag_has_value(candy_value_t *tag, candy_value_t *responsible)
{
    return to_candy_bool(tag->value.tag.value != NULL);
}
candy_value_t *candy_builtin_tag_get_value(candy_value_t *tag, candy_value_t *responsible)
{
    return tag->value.tag.value;
}
candy_value_t *candy_builtin_tag_without_value(candy_value_t *tag, candy_value_t *responsible)
{
    return make_candy_tag(tag->value.tag.text, NULL);
}

const candy_value_t *candy_builtin_type_of(candy_value_t *value, candy_value_t *responsible)
{
    switch (value->type)
    {
    case CANDY_TYPE_INT:
        return &__internal_int;
    case CANDY_TYPE_TEXT:
        return &__internal_text;
    case CANDY_TYPE_TAG:
        return &__internal_tag;
    case CANDY_TYPE_LIST:
        return &__internal_list;
    case CANDY_TYPE_STRUCT:
        return &__internal_struct;
    case CANDY_TYPE_FUNCTION:
        return &__internal_function;
    default:
        candy_panic(&__internal_unknown);
    }
}
