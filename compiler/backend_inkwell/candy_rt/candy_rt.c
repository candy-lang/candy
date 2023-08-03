#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include "candy_rt.h"
#include "candy_builtin.h"

candy_value_t __internal_true = {
    .value = {.text = "True"},
    .type = CANDY_TYPE_TAG};

candy_value_t __internal_false = {
    .value = {.text = "False"},
    .type = CANDY_TYPE_TAG};

candy_value_t __internal_nothing = {
    .value = {.text = "Nothing"},
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
    case CANDY_TYPE_LIST:
        printf("(");
        candy_value_t *length = candy_builtin_list_length(value);
        size_t list_length = length->value.integer;
        free_candy_value(length);
        size_t index = 0;
        for (size_t index = 0; index < list_length; index++)
        {
            print_candy_value(value->value.list[index]);
            if (index != list_length - 1)
            {
                printf(", ");
            }
        }
        printf(")");
        break;
    case CANDY_TYPE_FUNCTION:
        printf("Function %p", value->value.function.function);
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
    candy_value->value.text = malloc(sizeof(char) * (strlen(text) + 1));
    strcpy(candy_value->value.text, text);
    candy_value->type = CANDY_TYPE_TEXT;
    return candy_value;
}

candy_value_t *make_candy_tag(char *tag)
{
    candy_value_t *candy_value = malloc(sizeof(candy_value_t));
    candy_value->value.text = malloc(sizeof(char) * (strlen(tag) + 1));
    strcpy(candy_value->value.text, tag);
    candy_value->type = CANDY_TYPE_TAG;
    return candy_value;
}

candy_value_t *make_candy_list(candy_value_t **values)
{
    candy_value_t *candy_value = malloc(sizeof(candy_value_t));
    candy_value->value.list = values;
    candy_value->type = CANDY_TYPE_LIST;
    return candy_value;
}

candy_value_t *make_candy_function(candy_function function, void *environment, int env_size)
{
    candy_value_t *candy_value = malloc(sizeof(candy_value_t));
    candy_value->type = CANDY_TYPE_FUNCTION;
    candy_value->value.function.function = function;
    candy_value->value.function.environment = environment;
    return candy_value;
}

candy_value_t *make_candy_struct(candy_value_t **keys, candy_value_t **values)
{
    candy_value_t *candy_value = malloc(sizeof(candy_value_t));
    candy_value->type = CANDY_TYPE_STRUCT;
    candy_value->value.structure.keys = keys;
    candy_value->value.structure.values = values;
    return candy_value;
}

candy_value_t *call_candy_function_with(candy_value_t *function, candy_value_t *arg)
{
    return function->value.function.function(arg);
}

candy_function get_candy_function_pointer(candy_value_t *function)
{
    return function->value.function.function;
}

void *get_candy_function_environment(candy_value_t *function)
{
    return function->value.function.environment;
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
    if (value != candy_environment && value != NULL)
    {
        if (value->type == CANDY_TYPE_TAG || value->type == CANDY_TYPE_TEXT)
        {
            free(value->value.text);
        }
        // List and struct entries may not be freed as part of freeing
        // the list/struct, because they will be freed on their own
        // at the end of the main function.
        free(value);
    }
}