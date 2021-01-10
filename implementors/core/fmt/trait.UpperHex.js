(function() {var implementors = {};
implementors["bytes"] = [{"text":"impl UpperHex for Bytes","synthetic":false,"types":[]},{"text":"impl UpperHex for BytesMut","synthetic":false,"types":[]}];
implementors["env_logger"] = [{"text":"impl&lt;'a, T:&nbsp;UpperHex&gt; UpperHex for StyledValue&lt;'a, T&gt;","synthetic":false,"types":[]}];
implementors["generic_array"] = [{"text":"impl&lt;T:&nbsp;ArrayLength&lt;u8&gt;&gt; UpperHex for GenericArray&lt;u8, T&gt; <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;T: Add&lt;T&gt;,<br>&nbsp;&nbsp;&nbsp;&nbsp;&lt;T as Add&lt;T&gt;&gt;::Output: ArrayLength&lt;u8&gt;,&nbsp;</span>","synthetic":false,"types":[]}];
implementors["num_bigint"] = [{"text":"impl UpperHex for BigInt","synthetic":false,"types":[]},{"text":"impl UpperHex for BigUint","synthetic":false,"types":[]}];
implementors["tinyvec"] = [{"text":"impl&lt;A:&nbsp;Array&gt; UpperHex for ArrayVec&lt;A&gt; <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;A::Item: UpperHex,&nbsp;</span>","synthetic":false,"types":[]},{"text":"impl&lt;'s, T&gt; UpperHex for SliceVec&lt;'s, T&gt; <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;T: UpperHex,&nbsp;</span>","synthetic":false,"types":[]},{"text":"impl&lt;A:&nbsp;Array&gt; UpperHex for TinyVec&lt;A&gt; <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;A::Item: UpperHex,&nbsp;</span>","synthetic":false,"types":[]}];
implementors["uuid"] = [{"text":"impl UpperHex for Hyphenated","synthetic":false,"types":[]},{"text":"impl&lt;'a&gt; UpperHex for HyphenatedRef&lt;'a&gt;","synthetic":false,"types":[]},{"text":"impl UpperHex for Simple","synthetic":false,"types":[]},{"text":"impl&lt;'a&gt; UpperHex for SimpleRef&lt;'a&gt;","synthetic":false,"types":[]},{"text":"impl UpperHex for Urn","synthetic":false,"types":[]},{"text":"impl&lt;'a&gt; UpperHex for UrnRef&lt;'a&gt;","synthetic":false,"types":[]},{"text":"impl UpperHex for Uuid","synthetic":false,"types":[]}];
if (window.register_implementors) {window.register_implementors(implementors);} else {window.pending_implementors = implementors;}})()