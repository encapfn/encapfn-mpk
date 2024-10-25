// We should be careful in what we include here, and avoid having it pollute
// the implicit global namespace:
#include <stddef.h>

size_t ef_callback(size_t id, size_t arg0, size_t arg1, size_t arg2, size_t arg3);

char * getenv(const char *name);
void * malloc(size_t size);
void * calloc(size_t nitems, size_t size);
void * realloc(void * ptr, size_t newSize);
void free(void * ptr);

//void* malloc(size_t size);
/* int test_add(int a, int b); */
//void puts(char* arg);
