#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include "candy_runtime.h"
#include "candy_builtin.h"

const candy_value_t __internal_true = {
    .value = {.text = "True"},
    .type = CANDY_TYPE_TAG};

const candy_value_t __internal_false = {
    .value = {.text = "False"},
    .type = CANDY_TYPE_TAG};

const candy_value_t __internal_nothing = {
    .value = {.text = "Nothing"},
    .type = CANDY_TYPE_TAG,
    .refcount = 1};

const candy_value_t __internal_less = {
    .value = {.text = "Less"},
    .type = CANDY_TYPE_TAG};

const candy_value_t __internal_greater = {
    .value = {.text = "Greater"},
    .type = CANDY_TYPE_TAG};

const candy_value_t __internal_equal = {
    .value = {.text = "Equal"},
    .type = CANDY_TYPE_TAG};

const candy_value_t __internal_int = {.value = {.text = "Int"}, .type = CANDY_TYPE_TAG};
const candy_value_t __internal_text = {.value = {.text = "Text"}, .type = CANDY_TYPE_TAG};
const candy_value_t __internal_tag = {.value = {.text = "Tag"}, .type = CANDY_TYPE_TAG};
const candy_value_t __internal_list = {.value = {.text = "List"}, .type = CANDY_TYPE_TAG};
const candy_value_t __internal_struct = {.value = {.text = "Struct"}, .type = CANDY_TYPE_TAG};
const candy_value_t __internal_function = {.value = {.text = "Function"}, .type = CANDY_TYPE_TAG};
const candy_value_t __internal_unknown = {.value = {.text = "Unknown type"}, .type = CANDY_TYPE_TAG};

candy_value_t _candy_environment = {
    .value = {.text = "Environment"},
    .type = CANDY_TYPE_TAG,
    .refcount = 1};

// Not particularly elegant, but this is a temporary solution anyway...
candy_value_t *candy_environment = &_candy_environment;

void print_candy_value(const candy_value_t *value)
{
    switch (value->type)
    {
    case CANDY_TYPE_INT:
        printf("%ld", value->value.integer);
        break;
    case CANDY_TYPE_TEXT:
        printf("%s", value->value.text);
        break;
    case CANDY_TYPE_TAG:
        printf("%s", value->value.tag.text);
        if (value->value.tag.value)
        {
            printf(" ");
            print_candy_value(value->value.tag.value);
        }
        break;
    case CANDY_TYPE_LIST:
        printf("(");
        candy_value_t *length = candy_builtin_list_length(value);
        size_t list_length = length->value.integer;
        free_candy_value(length);
        size_t index = 0;
        switch (list_length)
        {
        case 1:
            print_candy_value(value->value.list[0]);
        case 0:
            printf(",");
            break;
        default:
            for (size_t index = 0; index < list_length; index++)
            {
                print_candy_value(value->value.list[index]);
                if (index != list_length - 1)
                {
                    printf(", ");
                }
            }
            break;
        }
        printf(")");
        break;
    case CANDY_TYPE_FUNCTION:
        printf("Function %p", value->value.function.function);
        break;
    case CANDY_TYPE_STRUCT:
        printf("[");
        printf("struct");
        printf("]");
        break;
    default:
        printf("<unknown type %d>", value->type);
        break;
    }
}

const candy_value_t *to_candy_bool(int value)
{
    return value ? &__internal_true : &__internal_false;
}

int candy_tag_to_bool(const candy_value_t *value)
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

candy_value_t *make_candy_int(int64_t value)
{
    candy_value_t *candy_value = malloc(sizeof(candy_value_t));
    candy_value->value.integer = value;
    candy_value->type = CANDY_TYPE_INT;
    candy_value->refcount = 1;
    return candy_value;
}

candy_value_t *make_candy_text(char *text)
{
    candy_value_t *candy_value = malloc(sizeof(candy_value_t));
    candy_value->value.text = malloc(sizeof(char) * (strlen(text) + 1));
    strcpy(candy_value->value.text, text);
    candy_value->type = CANDY_TYPE_TEXT;
    candy_value->refcount = 1;
    return candy_value;
}

candy_value_t *make_candy_tag(char *tag, candy_value_t *value)
{
    candy_value_t *candy_value = malloc(sizeof(candy_value_t));
    candy_value->value.tag.text = malloc(sizeof(char) * (strlen(tag) + 1));
    strcpy(candy_value->value.tag.text, tag);
    candy_value->value.tag.value = value;
    candy_value->type = CANDY_TYPE_TAG;
    candy_value->refcount = 1;
    return candy_value;
}

candy_value_t *make_candy_list(candy_value_t **values)
{
    candy_value_t *candy_value = malloc(sizeof(candy_value_t));
    candy_value->value.list = values;
    candy_value->type = CANDY_TYPE_LIST;
    candy_value->refcount = 1;
    return candy_value;
}

candy_value_t *make_candy_function(candy_function function, candy_value_t **captures, size_t capture_size)
{
    candy_value_t *candy_value = malloc(sizeof(candy_value_t));
    candy_value_t **captures_copy = malloc(capture_size);
    memcpy(captures_copy, captures, capture_size);
    candy_value->type = CANDY_TYPE_FUNCTION;
    candy_value->value.function.function = function;
    candy_value->value.function.captures = captures_copy;
    candy_value->refcount = 1;
    return candy_value;
}

candy_value_t *make_candy_struct(candy_value_t **keys, candy_value_t **values, size_t size)
{
    candy_value_t *candy_value = malloc(sizeof(candy_value_t));
    candy_value->type = CANDY_TYPE_STRUCT;
    candy_value->value.structure.keys = keys;
    candy_value->value.structure.values = values;
    candy_value->refcount = 1;
    return candy_value;
}

candy_value_t *run_candy_main(candy_value_t *function, candy_value_t *arg)
{
    return function->value.function.function(arg);
}

candy_function get_candy_function_ptr(candy_value_t *function)
{
    return function->value.function.function;
}

//candy_value_t *get_candy_function_capture(candy_value_t *function, int id)
//{
//    return function->value.function.captures[id];
//}

candy_value_t **get_candy_function_capture(candy_value_t *function)
{
    return function->value.function.captures;
}

void candy_panic(const candy_value_t *reason)
{
    printf("The program panicked for the following reason: \n");
    print_candy_value(reason);
    printf("\n");
    exit(-1);
}

void free_candy_value(candy_value_t *value)
{
    if (value == candy_environment || value == NULL)
    {
        return;
    }

    if (value->type == CANDY_TYPE_TAG || value->type == CANDY_TYPE_TEXT)
    {
        free(value->value.text);
    }
    // List and struct entries may not be freed as part of freeing
    // the list/struct, because they will be freed on their own
    // at the end of the main function.
    free(value);
}

void dup_candy_value(candy_value_t *value, int amount) {
    value->refcount += amount;
}

void drop_candy_value(candy_value_t *value) {
    value->refcount--;
    if (value->refcount == 0) {
        free_candy_value(value);
    }
}