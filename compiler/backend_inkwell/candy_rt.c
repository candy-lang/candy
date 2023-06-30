#include <stdint.h>
#include <stdlib.h>

#define int128_t long long int

typedef enum
{
    CANDY_TYPE_INT,
    CANDY_TYPE_TEXT,
    CANDY_TYPE_LIST,
    CANDY_TYPE_STRUCT,
} candy_type_t;

typedef struct candy_value
{
    union
    {
        int128_t integer;
        char *text;
        struct candy_value *list;
    } value;
    candy_type_t type;
} candy_value_t;

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