#ifndef __CANDY_RT_H
#define __CANDY_RT_H

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

typedef struct
{
    struct candy_value **keys;
    struct candy_value **values;
} candy_struct_t;

typedef struct candy_value
{
    union
    {
        int128_t integer;
        char *text;
        struct candy_value **list;
        candy_function_t function;
        candy_struct_t structure;
    } value;
    candy_type_t type;
} candy_value_t;

typedef candy_value_t *(*candy_function)(candy_value_t *);

extern candy_value_t __internal_true;
extern candy_value_t __internal_false;
extern candy_value_t _candy_environment;
extern candy_value_t *candy_environment;

void print_candy_value(candy_value_t *value);
candy_value_t *to_candy_bool(int value);
int candy_tag_to_bool(candy_value_t *value);
candy_value_t *make_candy_int(int128_t value);
candy_value_t *make_candy_text(char *text);
candy_value_t *make_candy_tag(char *tag);
candy_value_t *make_candy_list(candy_value_t **values);
candy_value_t *make_candy_function(candy_function function, candy_value_t *environment);
void candy_panic(candy_value_t *reason);
void free_candy_value(candy_value_t *value);
#endif