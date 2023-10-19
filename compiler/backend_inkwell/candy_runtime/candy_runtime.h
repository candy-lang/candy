#ifndef __CANDY_RT_H
#define __CANDY_RT_H

#include <stdint.h>

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
    void *environment;
    struct candy_value *(*function)(struct candy_value *, ...);
} candy_function_t;

typedef struct
{
    struct candy_value **keys;
    struct candy_value **values;
} candy_struct_t;

typedef struct
{
    char *text;
    struct candy_value *value;
} candy_tag_t;

typedef struct candy_value
{
    union
    {
        int64_t integer;
        char *text;
        candy_tag_t tag;
        struct candy_value **list;
        candy_function_t function;
        candy_struct_t structure;
    } value;
    candy_type_t type;
} candy_value_t;

typedef candy_value_t *(*candy_function)(candy_value_t *, ...);

const extern candy_value_t __internal_true;
const extern candy_value_t __internal_false;
const extern candy_value_t __internal_nothing;
const extern candy_value_t __internal_less;
const extern candy_value_t __internal_equal;
const extern candy_value_t __internal_greater;
const extern candy_value_t __internal_int;
const extern candy_value_t __internal_text;
const extern candy_value_t __internal_tag;
const extern candy_value_t __internal_list;
const extern candy_value_t __internal_struct;
const extern candy_value_t __internal_function;
const extern candy_value_t __internal_unknown;
const extern candy_value_t __internal_platform;
extern candy_value_t _candy_environment;
extern candy_value_t *candy_environment;

void print_candy_value(const candy_value_t *value);
const candy_value_t *to_candy_bool(int value);
int candy_tag_to_bool(const candy_value_t *value);
candy_value_t *make_candy_int(int64_t value);
candy_value_t *make_candy_text(char *text);
candy_value_t *make_candy_tag(char *tag, candy_value_t *value);
candy_value_t *make_candy_list(candy_value_t **values);
candy_value_t *make_candy_function(candy_function function, void *environment, int env_size);
candy_value_t *run_candy_main(candy_value_t *function, candy_value_t *arg);
candy_function get_candy_function_pointer(candy_value_t *function);
void *get_candy_function_environment(candy_value_t *function);
void candy_panic(const candy_value_t *reason);
void free_candy_value(candy_value_t *value);
#endif
