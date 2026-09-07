options(repos=c(CRAN="https://cloud.r-project.org"), timeout=180)
dir.create("target/R-libraries", recursive=TRUE, showWarnings=FALSE)
lib <- normalizePath("target/R-libraries", winslash="/")
.libPaths(c(lib,.libPaths()))
install.packages(c("TTR","jsonlite"), lib=lib, type="binary")
stopifnot(requireNamespace("TTR",quietly=TRUE),requireNamespace("jsonlite",quietly=TRUE))
versions <- installed.packages(lib.loc=lib)[,c("Package","Version","Built"),drop=FALSE]
write.csv(versions,"docs/evidence/ttr-oracle-packages.csv",row.names=FALSE)
print(versions)
