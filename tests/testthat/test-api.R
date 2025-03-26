test_that("version endpoint works", {
  endpoint <- porcelain::porcelain_package_endpoint("marathon", "GET", "/version")
  res <- endpoint$run()
  expect_equal(res$status_code, 200)
  expect_equal(res$data, scalar(package_version_string("porcelain")))
})
