/*
 * fpdf GLX compatibility shim.
 *
 * Some Mesa builds (observed with Mesa 18.3.4 over SSH X11 forwarding) raise
 * GLXBadContextTag from glXSwapIntervalMESA. GLX reports errors asynchronously,
 * so the failure surfaces later in XMapRaised and aborts the reader while the
 * window is being created. Report success for the Mesa swap interval request
 * and leave the default swap interval in place.
 *
 * Only the Mesa entry point is intercepted. Drivers that provide
 * glXSwapIntervalEXT keep their normal vertical sync behaviour.
 */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <stddef.h>
#include <string.h>

typedef void (*GlxExtProc)(void);
typedef GlxExtProc (*GlxGetProcAddress)(const unsigned char *name);
typedef void *(*Dlsym)(void *handle, const char *name);
typedef int (*SwapIntervalMesa)(unsigned int interval);

static int resolving;

/* libGL is often opened with RTLD_LOCAL, so RTLD_NEXT cannot see it. Remember
 * the handle the loader used and resolve the real entry point through it. */
static void *glx_handle;

static Dlsym real_dlsym(void) {
    static Dlsym next;
    if (!next) {
        int saved = resolving;
        resolving = 1;
        next = (Dlsym)dlvsym(RTLD_NEXT, "dlsym", "GLIBC_2.2.5");
        resolving = saved;
    }
    return next;
}

static void *next_symbol(const char *name) {
    Dlsym next = real_dlsym();
    void *symbol;
    int saved;
    if (!next) {
        return NULL;
    }
    saved = resolving;
    resolving = 1;
    symbol = next(glx_handle ? glx_handle : RTLD_NEXT, name);
    resolving = saved;
    return symbol;
}

static void remember_handle(void *handle) {
    if (handle && handle != RTLD_DEFAULT && handle != RTLD_NEXT) {
        glx_handle = handle;
    }
}

static GlxExtProc real_get_proc_address(const unsigned char *name) {
    static GlxGetProcAddress real;
    if (!real) {
        real = (GlxGetProcAddress)next_symbol("glXGetProcAddressARB");
    }
    if (!real) {
        real = (GlxGetProcAddress)next_symbol("glXGetProcAddress");
    }
    return real ? real(name) : NULL;
}

static int accept_swap_interval_mesa(unsigned int interval) {
    (void)interval;
    return 0;
}

GlxExtProc glXGetProcAddressARB(const unsigned char *name) {
    if (name && strcmp((const char *)name, "glXSwapIntervalMESA") == 0) {
        return (GlxExtProc)accept_swap_interval_mesa;
    }
    return real_get_proc_address(name);
}

GlxExtProc glXGetProcAddress(const unsigned char *name) {
    return glXGetProcAddressARB(name);
}

void *dlsym(void *handle, const char *name) {
    if (!resolving) {
        if (strcmp(name, "glXGetProcAddressARB") == 0) {
            remember_handle(handle);
            return (void *)glXGetProcAddressARB;
        }
        if (strcmp(name, "glXGetProcAddress") == 0) {
            remember_handle(handle);
            return (void *)glXGetProcAddress;
        }
        if (strcmp(name, "glXSwapIntervalMESA") == 0) {
            return (void *)accept_swap_interval_mesa;
        }
    }
    {
        Dlsym next = real_dlsym();
        return next ? next(handle, name) : NULL;
    }
}
