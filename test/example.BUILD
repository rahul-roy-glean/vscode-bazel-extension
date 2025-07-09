# Example BUILD file for testing the extension

go_library(
    name = "example_lib",
    srcs = ["example.go"],
    deps = [
        "//go/core:core",
        "@com_github_golang_protobuf//proto:go_default_library",
    ],
)

go_test(
    name = "example_test",
    srcs = ["example_test.go"],
    deps = [":example_lib"],
)

go_binary(
    name = "example_binary",
    srcs = ["main.go"],
    deps = [":example_lib"],
)