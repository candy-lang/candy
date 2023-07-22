#include <string.h>
#include "candy_rt.h"

candy_value_t *candy_builtin_equals(candy_value_t *left, candy_value_t *right)
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

candy_value_t *candy_builtin_ifelse(candy_value_t *condition, candy_value_t *then, candy_value_t *otherwise)
{
    if (candy_tag_to_bool(condition))
    {
        candy_function then_function = (then->value).function.function;
        candy_value_t *environment = (then->value).function.environment;
        return then_function(environment);
    }
    else
    {
        candy_function otherwise_function = (otherwise->value).function.function;
        candy_value_t *environment = (otherwise->value).function.environment;
        return otherwise_function(environment);
    }
}

candy_value_t *candy_builtin_int_add(candy_value_t *left, candy_value_t *right)
{
    return make_candy_int(left->value.integer + right->value.integer);
}

candy_value_t *candy_builtin_int_subtract(candy_value_t *left, candy_value_t *right)
{
    return make_candy_int(left->value.integer - right->value.integer);
}

candy_value_t *candy_builtin_int_bit_length(candy_value_t *value)
{
    return make_candy_int(64);
}

candy_value_t *candy_builtin_int_bitwise_and(candy_value_t *left, candy_value_t *right)
{
    return make_candy_int(left->value.integer & right->value.integer);
}

candy_value_t *candy_builtin_int_bitwise_or(candy_value_t *left, candy_value_t *right)
{
    return make_candy_int(left->value.integer | right->value.integer);
}

candy_value_t *candy_builtin_int_bitwise_xor(candy_value_t *left, candy_value_t *right)
{
    return make_candy_int(left->value.integer ^ right->value.integer);
}

candy_value_t *candy_builtin_int_compareto(candy_value_t *left, candy_value_t *right)
{
    int128_t left_value = left->value.integer;
    int128_t right_value = right->value.integer;
    if (left_value < right_value)
    {
        return make_candy_tag("Less");
    }
    else if (left_value == right_value)
    {
        return make_candy_tag("Equal");
    }
    else
    {
        return make_candy_tag("Greater");
    }
}

candy_value_t *candy_builtin_list_length(candy_value_t *list)
{
    size_t index = 0;
    while (list->value.list[index] != NULL)
    {
        index++;
    }
    return make_candy_int(index);
}

candy_value_t *candy_builtin_struct_get(candy_value_t *structure, candy_value_t *key)
{
    size_t index = 0;
    while (!candy_tag_to_bool(candy_builtin_equals(structure->value.structure.keys[index], key)))
    {
        if (structure->value.structure.keys[index] == NULL)
        {
            candy_panic(make_candy_text("Attempted to access non-existent struct member"));
        }
        index++;
    }
    return structure->value.structure.values[index];
}

candy_value_t *candy_builtin_struct_get_keys(candy_value_t *structure)
{
    return make_candy_list(structure->value.structure.keys);
}

candy_value_t *candy_builtin_struct_has_key(candy_value_t *structure, candy_value_t *key)
{

    size_t index = 0;
    while (structure->value.structure.keys[index] != NULL)
    {
        if (candy_tag_to_bool(candy_builtin_equals(structure->value.structure.keys[index], key)))
        {
            return &__internal_true;
        }
    }
    return &__internal_false;
}

candy_value_t *candy_builtin_typeof(candy_value_t *value)
{
    switch (value->type)
    {
    case CANDY_TYPE_INT:
        return make_candy_tag("Int");
    case CANDY_TYPE_TEXT:
        return make_candy_tag("Text");
    case CANDY_TYPE_TAG:
        return make_candy_tag("Tag");
    case CANDY_TYPE_LIST:
        return make_candy_tag("List");
    case CANDY_TYPE_STRUCT:
        return make_candy_tag("Struct");
    case CANDY_TYPE_FUNCTION:
        return make_candy_tag("Function");
    }
}