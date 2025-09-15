#' @porcelain
#'   GET /version => json("string")
version <- function() {
  scalar(package_version_string("porcelain"))
}


api <- function(validate = FALSE) {
  api <- porcelain::porcelain$new(validate = validate)
  api$include_package_endpoints()
  api
}
