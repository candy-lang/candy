#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>

#define int128_t long long int

typedef enum
{
    CANDY_TYPE_INT = 42,
    CANDY_TYPE_TEXT,
    CANDY_TYPE_TAG,
    CANDY_TYPE_LIST,
    CANDY_TYPE_STRUCT,
    CANDY_TYPE_FUNCTION,
} candy_type_t;

typedef struct candy_value
{
    union
    {
        int128_t integer;
        char *text;
        struct candy_value *list;
        struct candy_value *(*function)(void);
    } value;
    candy_type_t type;
} candy_value_t;

typedef candy_value_t *(*candy_function)(void);

candy_value_t __internal_true = {
    .value = {.text = "True"},
    .type = CANDY_TYPE_TAG};

candy_value_t __internal_false = {
    .value = {.text = "False"},
    .type = CANDY_TYPE_TAG};

candy_value_t _candy_environment = {
    .value = {.text = "Environment"},
    .type = CANDY_TYPE_TAG};

// Not particularly elegant, but this is a temporary solution anyway...
candy_value_t *candy_environment = &_candy_environment;

candy_value_t *to_candy_bool(int value)
{
    if (value)
    {
        return &__internal_true;
    }
    else
    {
        return &__internal_false;
    }
}

void print_candy_value(candy_value_t *value)
{
    switch (value->type)
    {
    case CANDY_TYPE_INT:
        printf("%lld", value->value.integer);
        break;
    case CANDY_TYPE_TEXT:
        printf("%s", value->value.text);
        break;
    case CANDY_TYPE_TAG:
        printf("%s", value->value.text);
        break;
    default:
        printf("<unknown type %d>", value->type);
        break;
    }
}

candy_value_t *make_candy_int(int128_t value)
{
    candy_value_t *candy_value = malloc(sizeof(candy_value_t));
    candy_value->value.integer = value;
    candy_value->type = CANDY_TYPE_INT;
    return candy_value;
}

candy_value_t *make_candy_text(char *text)
{
    candy_value_t *candy_value = malloc(sizeof(candy_value_t));
    candy_value->value.text = text;
    candy_value->type = CANDY_TYPE_TEXT;
    return candy_value;
}

candy_value_t *make_candy_tag(char *tag)
{
    candy_value_t *candy_value = malloc(sizeof(candy_value_t));
    candy_value->value.text = tag;
    candy_value->type = CANDY_TYPE_TAG;
    return candy_value;
}

candy_value_t *make_candy_function(candy_function function)
{
    candy_value_t *candy_value = malloc(sizeof(candy_value_t));
    candy_value->type = CANDY_TYPE_FUNCTION;
    candy_value->value.function = function;
    return candy_value;
}

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

candy_value_t *candy_builtin_ifelse(candy_type_t *condition, candy_value_t *then, candy_value_t *otherwise)
{
    if (condition)
    {
        return then->value.function();
    }
    else
    {
        return otherwise->value.function();
    }
}

candy_value_t *candy_builtin_typeof(candy_value_t *value)
{
    switch (value->type)
    {
    case CANDY_TYPE_INT:
        return make_candy_tag("int");
    case CANDY_TYPE_TEXT:
        return make_candy_tag("text");
    case CANDY_TYPE_TAG:
        return make_candy_tag("tag");
    case CANDY_TYPE_LIST:
        return make_candy_tag("list");
    case CANDY_TYPE_STRUCT:
        return make_candy_tag("struct");
    case CANDY_TYPE_FUNCTION:
        return make_candy_tag("function");
    }
}

void candy_panic(candy_value_t *reason)
{
    printf("The program panicked for the following reason: \n");
    print_candy_value(reason);
    printf("\n");
    abort();
}
