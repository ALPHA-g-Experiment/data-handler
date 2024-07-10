document.querySelectorAll('.dropdown-menu a').forEach(function(element) {
	let nextEl = element.nextElementSibling;
	if(nextEl && nextEl.classList.contains('sub-menu')) {
		element.addEventListener('click', function (e) {
			// prevent opening link if link needs to open dropdown
			e.preventDefault();
			e.stopPropagation();
		});
	}
})

const ws = new WebSocket("ws://" + window.location.host + "/ws");

ws.onopen = function() {
	button = document.getElementById("downloadButton");
	button.removeAttribute("disabled");
}

ws.onclose = function() {
	button = document.getElementById("downloadButton");
	button.setAttribute("disabled", "disabled");
}

ws.onmessage = function(event) {
	const msg = JSON.parse(event.data);

	switch (msg.service) {
		case "Download":
			const outputId = msg.context;
			var output = document.getElementById(outputId);
			if (output === null) {
				break;
			}

			if (msg.response.Text) {
				output.textContent += msg.response.Text + "\n";
			} else if (msg.response.Error) {
				output.textContent += msg.response.Error + "\n";
			} else if (msg.response.DownloadJWT) {
				var a = document.createElement("a");
				a.href = "/download/" + msg.response.DownloadJWT;
				a.setAttribute("download", "");
				a.click();

				output.textContent += "Done\n";
			}

			break;
	}
}

function loadUrl() {
	var baseUrl = window.location.origin;
	var searchInput = document.getElementById("search-input").value;
	window.location.href = baseUrl + "/" + encodeURIComponent(searchInput);
}

document.getElementById("load-button").addEventListener("click", loadUrl);
document.getElementById("search-input").addEventListener("keypress",function(e) {
	if (e.key === "Enter") {
		loadUrl();
	}
});

let tabCounter = 1;

function createNewTab(jsonMessage) {
	var tabId = 'tab-' + tabCounter;
	var tabContentId = 'content-' + tabId;

	var tabList = document.getElementById("commandTabs");
	var tabContent = document.getElementById("commandTabContent");

	var newTab = document.createElement("li");
	newTab.className = "nav-item";
	newTab.innerHTML = `
		<a class="nav-link d-flex align-items-center" id="${tabId}"	data-bs-toggle="tab" href="#${tabContentId}" role="tab">
			${tabCounter} 
			<button class="btn-close ms-2" aria-label="Close" onclick="closeTab(event, '${tabId}', '${tabContentId}')"></button>
		</a>`;
	tabList.appendChild(newTab);

	var outputId = 'output-' + tabContentId;
	var newTabContent = document.createElement("div");
	newTabContent.className = "tab-pane fade";
	newTabContent.id = tabContentId;
	newTabContent.setAttribute("role", "tabpanel");
	newTabContent.innerHTML = `<pre class="p-3 bg-light border" id="${outputId}"></pre>`;
	tabContent.appendChild(newTabContent);

	var tabTrigger = new bootstrap.Tab(document.getElementById(`${tabId}`));
	tabTrigger.show();

	tabCounter++;

	jsonMessage.context = outputId;
	ws.send(JSON.stringify(jsonMessage));
}

function closeTab(event, tabId, tabContentId) {
	event.stopPropagation();
	document.getElementById(tabId).parentElement.remove();
	document.getElementById(tabContentId).remove();
	
	var remainingTabs = document.querySelectorAll('#commandTabs .nav-link');
	if (remainingTabs.length > 0) {
		var firstTab = new bootstrap.Tab(remainingTabs[0]);
		firstTab.show();
	}
}

function requestTrgScalersPlot() {
	const trg_args = {
		t_bins: parseInt(document.getElementById("TRGtBins").value),
		t_max: parseFloat(document.getElementById("TRGtMax").value),
		t_min: parseFloat(document.getElementById("TRGtMin").value),
		include_drift_veto: document.getElementById("TRGDriftVeto").checked,
		include_pulser: document.getElementById("TRGPulser").checked,
		include_scaledown: document.getElementById("TRGScaleDown").checked,
		remove_input: !document.getElementById("TRGInput").checked,
		remove_output: !document.getElementById("TRGOutput").checked,
	};

	createNewTab({service: 'Download', context: '', request: {TrgScalersPlot: {run_number: var_run_number, args: trg_args}}});
}

function requestVtxPlot() {
	const vtx_args = {
		phi_bins: parseInt(document.getElementById("phi_bins").value),
		phi_max: parseFloat(document.getElementById("phi_max").value),
		phi_min: parseFloat(document.getElementById("phi_min").value),
		r_bins: parseInt(document.getElementById("r_bins").value),
		r_max: parseFloat(document.getElementById("r_max").value),
		r_min: parseFloat(document.getElementById("r_min").value),
		t_bins: parseInt(document.getElementById("t_bins").value),
		t_max: parseFloat(document.getElementById("t_max").value),
		t_min: parseFloat(document.getElementById("t_min").value),
		z_bins: parseInt(document.getElementById("z_bins").value),
		z_max: parseFloat(document.getElementById("z_max").value),
		z_min: parseFloat(document.getElementById("z_min").value),
	};

	createNewTab({service: 'Download', context: '', request: {VerticesPlot: {run_number: var_run_number, args: vtx_args}}});
}

function requestChronoboxPlot() {
	const cb_args = {
		board_name: document.getElementById("boardName").value,
		channel_number: parseInt(document.getElementById("channelNumber").value),
		t_bins: parseInt(document.getElementById("cb_tBins").value),
		t_max: parseFloat(document.getElementById("cb_tMax").value),
		t_min: parseFloat(document.getElementById("cb_tMin").value),
	};

	createNewTab({service: 'Download', context: '', request: {ChronoboxPlot: {run_number: var_run_number, args: cb_args}}});
}
