#include <unistd.h>

#define MAX_TRACES 1024

typedef struct {
    int call_site;
    int function;
    int *args;
    int responsible;
} candy_call_t;

typedef struct
{
    candy_call_t calls[MAX_TRACES];
    size_t idx;
} candy_tracer_t;

candy_tracer_t candy_default_tracer_v = {.calls = {0}, .idx = 0};
candy_tracer_t *candy_default_tracer = &candy_default_tracer_v;

void trace_call_starts(candy)