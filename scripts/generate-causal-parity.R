# Independent prefix observations: never execute the Rust implementation.
.libPaths(c(normalizePath('target/R-libraries'),.libPaths()))
suppressPackageStartupMessages(library(TTR))
oracle <- new.env(parent=asNamespace('TTR'))
for (file in list.files('target/cross-library/ttr/R',pattern='[.]R$',full.names=TRUE)) sys.source(file,oracle)
dll <- dyn.load(normalizePath('target/cross-library/ttr-msvc/build/Release/TTR.dll'))
routines <- getDLLRegisteredRoutines(dll)$.Call
for (name in names(routines)) assign(paste0('C_',name),routines[[name]],oracle)
data <- jsonlite::fromJSON('crates/roze-ta/tests/fixtures/ttr-parity.json',simplifyVector=FALSE)
records <- list()
for (f in data$fixtures) {
  if (!(f$name %in% c('ZigZag','DPO'))) next
  bars <- data$datasets[[f$dataset]]
  hl <- do.call(rbind,lapply(bars,function(s) unlist(s$value[c('high','low')])))
  if(f$name=='ZigZag') {
    # The native oracle reads two initial bars: one-bar input is explicitly warming.
    f$expected <- lapply(seq_len(nrow(hl)),function(i) if(i<2) NA_real_ else tail(oracle$ZigZag(hl[seq_len(i),,drop=FALSE],change=f$request$operation$params$threshold*100),1))
    f$alignment <- 'prefix_snapshot_last; one sample is warming (reference requires two)'
  } else {
    delay <- floor(f$request$operation$params$period/2)+1
    f$expected <- c(rep(list(NA_real_),delay),head(f$expected,-delay))
    f$alignment <- paste0('delayed_by_',delay,'_observations; source value t emitted at t+delay')
  }
  records[[length(records)+1]] <- f
}
jsonlite::write_json(records,'target/causal-ttr.json',auto_unbox=TRUE,digits=NA,na='null',null='null')
