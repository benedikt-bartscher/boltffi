#include <stdint.h>

#include "test.h"

static uint32_t callback_releases;

static void value_callback_free(uint64_t identity) {
    if (identity == 42) ++callback_releases;
}

static uint64_t value_callback_clone(uint64_t identity) {
    return identity;
}

static int32_t value_callback_invoke(uint64_t identity, int32_t value) {
    return value + (int32_t)identity;
}

bool test_callback_ownership(void) {
    static const DemoValueCallback vtable = {
        .free = value_callback_free,
        .clone = value_callback_clone,
        .on_value = value_callback_invoke
    };
    DemoValueCallbackHandle callback = demo_value_callback_create(&vtable, 42);
    DemoValueCallbackHandle copy = demo_value_callback_clone(&callback);
    CHECK(demo_invoke_value_callback(&callback, 1) == 43, "foreign callback invocation");
    CHECK(callback.raw.handle == 0 && callback.raw.vtable == NULL && callback_releases == 1, "consumed foreign callback is cleared");
    demo_value_callback_free(&callback);
    CHECK(callback_releases == 1, "cleared callback cannot be freed twice");
    CHECK(demo_invoke_boxed_value_callback(&copy, 2) == 44, "cloned callback survives the original");
    CHECK(copy.raw.handle == 0 && callback_releases == 2, "consumed clone is released once");
    CHECK(demo_invoke_optional_value_callback(NULL, 7) == 7, "absent callback uses the Rust fallback");
    callback = demo_make_incrementing_callback(5);
    copy = demo_value_callback_clone(&callback);
    CHECK(demo_invoke_boxed_value_callback(&copy, 37) == 42, "Rust callback round trip");
    CHECK(copy.raw.handle == 0, "consumed Rust callback is cleared");
    CHECK(demo_invoke_value_callback(&callback, 2) == 7, "Rust callback clone retains the original");
    demo_value_callback_free(&callback);
    callback = demo_make_incrementing_callback(6);
    demo_value_callback_free(&callback);
    CHECK(callback.raw.handle == 0 && callback.raw.vtable == NULL, "returned Rust callback is explicitly releasable");
    return true;
}
