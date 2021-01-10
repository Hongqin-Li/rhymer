(function() {var implementors = {};
implementors["num_bigint"] = [{"text":"impl Neg for Sign","synthetic":false,"types":[]},{"text":"impl Neg for BigInt","synthetic":false,"types":[]},{"text":"impl&lt;'a&gt; Neg for &amp;'a BigInt","synthetic":false,"types":[]},{"text":"impl Neg for BigUint","synthetic":false,"types":[]},{"text":"impl&lt;'a&gt; Neg for &amp;'a BigUint","synthetic":false,"types":[]}];
implementors["time"] = [{"text":"impl Neg for Duration","synthetic":false,"types":[]}];
implementors["typenum"] = [{"text":"impl Neg for Z0","synthetic":false,"types":[]},{"text":"impl&lt;U:&nbsp;Unsigned + NonZero&gt; Neg for PInt&lt;U&gt;","synthetic":false,"types":[]},{"text":"impl&lt;U:&nbsp;Unsigned + NonZero&gt; Neg for NInt&lt;U&gt;","synthetic":false,"types":[]},{"text":"impl Neg for ATerm","synthetic":false,"types":[]},{"text":"impl&lt;V, A&gt; Neg for TArr&lt;V, A&gt; <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;V: Neg,<br>&nbsp;&nbsp;&nbsp;&nbsp;A: Neg,&nbsp;</span>","synthetic":false,"types":[]}];
if (window.register_implementors) {window.register_implementors(implementors);} else {window.pending_implementors = implementors;}})()