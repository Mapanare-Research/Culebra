// Generated C from Mapanare v3.0.0 C backend (fixture for testing)
// Contains common patterns the C emitter produces

#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>

// --- Structs ---
typedef struct {
    char* ptr;
    int64_t len;
} MnString;

typedef struct {
    void* data;
    int64_t len;
    int64_t cap;
    int64_t elem_size;
} MnList;

// --- Enum with tagged union ---
typedef struct {
    int64_t tag;
    union {
        struct { int64_t value; } IntLit;
        struct { MnString value; } StrLit;
        struct { double value; } FloatLit;
    } data;
} Expr;

// --- BUG: missing typedef for Token (used but not defined) ---
// Token used in function but no typedef

// --- Function with switch fallthrough bug ---
MnString expr_to_string(Expr e) {
    switch (e.tag) {
        case 0:
            // IntLit — BUG: no break, falls through to StrLit
            return (MnString){.ptr = "int", .len = 3};
        case 1:
            return (MnString){.ptr = "str", .len = 3};
            break;
        case 2:
            return e.data.StrLit.value;  // BUG: wrong union member (tag 2 is FloatLit)
            break;
    }
    return (MnString){.ptr = "unknown", .len = 7};
}

// --- Function with goto ---
int process_list(MnList* list) {
    int64_t i = 0;
    loop_start:
    if (i >= list->len) goto loop_end;
    void* elem = (char*)list->data + i * list->elem_size;
    if (elem == NULL) goto error;
    i++;
    goto loop_start;
    error:
    return -1;
    loop_end:
    return 0;
}

// --- Large struct by value ---
typedef struct {
    MnString name;
    MnList functions;
    MnList structs;
    MnList enums;
    MnList extern_fns;
    MnList agents;
    MnList pipes;
    MnList imports;
    MnList trait_names;
} MIRModule;

// BUG: large struct passed by value (should be pointer)
MIRModule create_module(MnString name) {
    MIRModule m;
    memset(&m, 0, sizeof(MIRModule));
    m.name = name;
    return m;
}

// --- Direct array access (should use __mn_list_get) ---
int64_t get_element(MnList list, int64_t idx) {
    int64_t* arr = (int64_t*)list.data;
    return arr[idx];  // BUG: no bounds check
}

// --- Missing return on some paths ---
int classify(int x) {
    if (x > 0) {
        return 1;
    } else if (x < 0) {
        return -1;
    }
    // BUG: no return for x == 0
}

// Runtime declarations
extern void __mn_str_println(MnString s);
extern MnList __mn_list_new(int64_t elem_size);
extern void __mn_list_push(MnList* list, void* elem, int64_t size);
extern void* __mn_list_get(MnList* list, int64_t idx);

int main(void) {
    MnString hello = {.ptr = "hello", .len = 5};
    __mn_str_println(hello);
    return 0;
}
