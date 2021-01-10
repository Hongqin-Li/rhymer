(function() {var implementors = {};
implementors["futures_channel"] = [{"text":"impl&lt;T&gt; FusedStream for Receiver&lt;T&gt;","synthetic":false,"types":[]},{"text":"impl&lt;T&gt; FusedStream for UnboundedReceiver&lt;T&gt;","synthetic":false,"types":[]}];
implementors["futures_core"] = [];
implementors["futures_intrusive"] = [{"text":"impl&lt;MutexType, T, A&gt; FusedStream for SharedStream&lt;MutexType, T, A&gt; <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;MutexType: RawMutex,<br>&nbsp;&nbsp;&nbsp;&nbsp;A: 'static + RingBuf&lt;Item = T&gt;,&nbsp;</span>","synthetic":false,"types":[]}];
if (window.register_implementors) {window.register_implementors(implementors);} else {window.pending_implementors = implementors;}})()