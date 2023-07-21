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

typedef struct
{
    struct candy_value *environment;
    struct candy_value *(*function)(struct candy_value *);
} candy_function_t;

typedef struct candy_value
{
    union
    {
        int128_t integer;
        char *text;
        struct candy_value *list;
        candy_function_t function;
    } value;
    candy_type_t type;
} candy_value_t;

typedef candy_value_t *(*candy_function)(candy_value_t *);

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

int candy_tag_to_bool(candy_value_t *value)
{
    if (strcmp(value->value.text, "True") == 0)
    {
        return 1;
    }
    else if (strcmp(value->value.text, "False") == 0)
    {
        return 0;
    }
    else
    {
        printf("Got invalid value ");
        print_candy_value(value);
        printf("\n");
        exit(-1);
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
    candy_value->value.text = malloc(sizeof(char) * strlen(text));
    strcpy(candy_value->value.text, text);
    candy_value->type = CANDY_TYPE_TEXT;
    return candy_value;
}

candy_value_t *make_candy_tag(char *tag)
{
    candy_value_t *candy_value = malloc(sizeof(candy_value_t));
    candy_value->value.text = malloc(sizeof(char) * strlen(tag));
    strcpy(candy_value->value.text, tag);
    candy_value->type = CANDY_TYPE_TAG;
    return candy_value;
}

candy_value_t *make_candy_function(candy_function function, candy_value_t *environment)
{
    candy_value_t *candy_value = malloc(sizeof(candy_value_t));
    candy_value->type = CANDY_TYPE_FUNCTION;
    candy_value->value.function.function = function;
    candy_value->value.function.environment = environment;
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
    // This is the max size in the VM. Unsure if it applies here.
    return make_candy_int(62);
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

void candy_panic(candy_value_t *reason)
{
    printf("The program panicked for the following reason: \n");
    print_candy_value(reason);
    printf("\n");
    exit(-1);
}

void free_candy_value(candy_value_t *value)
{
    if (value != candy_environment)
    {
        if (value->type == CANDY_TYPE_TAG || value->type == CANDY_TYPE_TEXT)
        {
            free(value->value.text);
        }
        free(value);
    }
}