# Test-only oracle: pinned R functions and independently compiled pinned C DLL.
.libPaths(c(normalizePath('target/R-libraries'),.libPaths()))
suppressPackageStartupMessages(library(TTR))
oracle <- new.env(parent=asNamespace('TTR'))
for (file in list.files('target/cross-library/ttr/R',pattern='[.]R$',full.names=TRUE)) sys.source(file,oracle)
dll <- dyn.load(normalizePath('target/cross-library/ttr-msvc/build/Release/TTR.dll'))
routines <- getDLLRegisteredRoutines(dll)$.Call
for (name in names(routines)) assign(paste0('C_',name),routines[[name]],oracle)
data <- jsonlite::fromJSON('target/ttr-parity-input.json',simplifyVector=FALSE)
records <- list()
for (case in names(data$datasets)) {
  candles <- do.call(rbind,lapply(data$datasets[[case]],unlist))
  close <- candles[,'close']; volume <- candles[,'volume']; nrows <- length(close)
  secondary <- close*.8+cos(seq_len(nrows)*.37)
  for (row in data$entries) {
    name <- row$name; p <- row$parameters; args <- list()
    record <- list(source='ttr',name=name,case=case,operation_id=row$operation_id)
    outcome <- tryCatch({
      fn <- get(name,oracle); fm <- names(formals(fn))
      inputs <- list(x=close,y=secondary,price=close,prices=close,volume=volume,HLC=candles[,c('high','low','close')],HL=candles[,c('high','low')],OHLC=candles[,c('open','high','low','close')],Ra=close,Rb=secondary,signals=secondary)
      # BBands accepts an explicitly selected scalar price, matching its local input.
      if(name=='BBands') inputs$HLC <- close
      if(name=='rollSFM') {inputs$Ra<-xts::xts(close,as.Date('2020-01-01')+seq_len(nrows));inputs$Rb<-xts::xts(secondary,as.Date('2020-01-01')+seq_len(nrows))}
      args <- inputs[intersect(fm,names(inputs))]
      if(name=='runVar') args$y <- NULL
      if('n' %in% fm && !is.null(p$period)) args$n <- p$period
      if(name=='ALMA') {args$offset<-p$offset;args$sigma<-p$sigma}
      if(name=='BBands') args$sd<-p$multiplier
      if(name=='KST') {args$n<-unlist(p[paste0('sma',1:4)]);args$nROC<-unlist(p[paste0('roc',1:4)]);args$nSig<-p$signal}
      if(name=='MACD') {args$nFast<-p$fast;args$nSlow<-p$slow;args$nSig<-p$signal}
      if(name=='SMI') {args$nFast<-p$d_period;args$nSlow<-p$d2_period}
      if(name=='SAR') args$accel<-c(p$af_step,p$af_max)
      if(name=='ZigZag') args$change<-p$threshold*100
      if(name=='chaikinVolatility') args$n<-p$ema_period
      if(name=='keltnerChannels') {args$n<-p$ema_period;args$atr<-p$multiplier}
      if(name=='stoch') {args$nFastK<-p$k_period;args$nFastD<-p$d_period;args$nSlowD<-p$d_period}
      if(name=='ultimateOscillator') args$n<-c(p$short,p$mid,p$long)
      if(name=='volatility') args$N<-p$trading_periods
      out<-do.call(fn,args); out<-as.matrix(out)
      if(name=='lags') out<-rbind(matrix(NA_real_,nrow=p$period,ncol=ncol(out),dimnames=list(NULL,colnames(out))),out)
      stopifnot(nrow(out)==nrows)
      output<-lapply(seq_len(nrows),function(i) if(ncol(out)==1) unname(out[i,1]) else as.list(setNames(as.numeric(out[i,]),if(is.null(colnames(out))) paste0('V',seq_len(ncol(out))) else colnames(out))))
      list(status='reference_generated',parameters=args[setdiff(names(args),names(inputs))],expected=output)
    },error=function(e) list(status='oracle_error',error=conditionMessage(e)))
    records[[length(records)+1]]<-c(record,outcome)
  }
}
jsonlite::write_json(list(schema_version=1,oracle=list(r_sources='76e5618e4a714f190519ee98da6de3234cf7d8b5',native_sources='76e5618e4a714f190519ee98da6de3234cf7d8b5',note='MSVC test-only native build; R header compatibility flags select int enum and unused legacy complex layout; source formulas unchanged.'),records=records),'target/ttr-parity-output.json',auto_unbox=TRUE,digits=NA,na='null',null='null')
print(table(vapply(records,function(r)r$status,'')))
