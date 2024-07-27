document
  .getElementById("searchInput")
  .addEventListener("keypress", function (e) {
    if (e.key === "Enter") {
      const input = document.getElementById("searchInput").value;
      if (input) {
        window.location.href = "./" + encodeURIComponent(input);
      }
    }
  });

document.querySelectorAll(".dropdown-menu button").forEach(function (element) {
  const nextElement = element.nextElementSibling;
  if (nextElement && nextElement.classList.contains("sub-menu")) {
    element.addEventListener("click", function (e) {
      e.preventDefault();
      e.stopPropagation();
    });
  }
});

const loc = window.location;
const protocol = loc.protocol === "https:" ? "wss:" : "ws:";
const ws = new WebSocket(protocol + "//" + loc.host + loc.pathname + "/../ws");

ws.onopen = function () {
  document.getElementById("downloadButton").disabled = false;
};

// The WebSocket connection is closed when the page is unloaded. In this case,
// we don't want to show an alert.
let isPageUnloading = false;
window.addEventListener("beforeunload", function () {
  isPageUnloading = true;
});

ws.onclose = function () {
  document.getElementById("downloadButton").disabled = true;
  if (
    !isPageUnloading &&
    !alert(
      "Connection to the server was lost.\n\nThe page will reload when you close this alert. Please request any data or plots you were waiting for again.",
    )
  ) {
    window.location.reload();
  }
};

ws.onmessage = function (event) {
  const msg = JSON.parse(event.data);

  switch (msg.service) {
    case "Download":
      handleDownloadResponse(msg);
      break;
  }
};

function handleDownloadResponse(msg) {
  const output = document.getElementById(msg.context);
  if (output === null) {
    return;
  }

  if (msg.response.Text) {
    output.textContent += msg.response.Text + "\n";
  } else if (msg.response.Error) {
    output.textContent += msg.response.Error + "\n";
  } else if (msg.response.DownloadJWT) {
    const a = document.createElement("a");
    a.href = "./download/" + msg.response.DownloadJWT;
    a.setAttribute("download", "");
    a.click();

    output.textContent += "Done\n";
  }
}

let tabCounter = 1;

function newDownload(jsonMessage) {
  const newTabId = "tab-" + tabCounter;
  const newTabContentId = "tabContent-" + tabCounter;
  const newTabContentOutputId = "tabContentOutput-" + tabCounter;

  document.getElementById("downloadTabs").innerHTML += `
    <li class="nav-item" id="${newTabId}">
      <div
        class="nav-link pe-2"
        data-bs-toggle="tab"
        data-bs-target="#${newTabContentId}"
        role="button"
      >
        ${tabCounter}
        <button
          type="button"
          class="btn-close tab-btn-close align-middle ms-1"
          onclick="
            document.getElementById('${newTabId}').remove();
            document.getElementById('${newTabContentId}').remove();

            const tabs = document.querySelectorAll('#downloadTabs .nav-link');
            if (tabs.length > 0) {
              new bootstrap.Tab(tabs[0]).show();
            }
          "
        ></button>
      </div>
    </li>
  `;

  document.getElementById("downloadTabsContent").innerHTML += `
    <div class="tab-pane fade" id="${newTabContentId}" tabindex="0">
      <pre
        class="p-3 bg-light border border-top-0"
        id="${newTabContentOutputId}"
      ></pre>
    </div>
  `;

  new bootstrap.Tab(
    document.getElementById(newTabId).querySelector(".nav-link"),
  ).show();

  jsonMessage.context = newTabContentOutputId;
  ws.send(JSON.stringify(jsonMessage));

  tabCounter++;
}

new TomSelect("#chronoboxChannel", {
  maxOptions: null,
  selectOnTab: true,
});

function updateChronoboxMaxTimeReq() {
  document.getElementById("chronoboxMaxT").min =
    document.getElementById("chronoboxMinT").value;
}

function chronoboxPlot() {
  updateChronoboxMaxTimeReq();

  if (!document.getElementById("chronoboxForm").reportValidity()) {
    return;
  }

  const channel = JSON.parse(document.getElementById("chronoboxChannel").value);
  const chronoboxArgs = {
    board_name: channel.board,
    channel_number: parseInt(channel.number),
    t_bins: parseInt(document.getElementById("chronoboxBins").value),
    t_max: parseFloat(document.getElementById("chronoboxMaxT").value),
    t_min: parseFloat(document.getElementById("chronoboxMinT").value),
  };

  newDownload({
    service: "Download",
    context: "",
    request: { ChronoboxPlot: { run_number: RUN_NUMBER, args: chronoboxArgs } },
  });

  bootstrap.Modal.getInstance(document.getElementById("chronoboxModal")).hide();
}

function updateTrgMaxTimeReq() {
  document.getElementById("trgMaxT").min =
    document.getElementById("trgMinT").value;
}

function updateTrgScalersReq() {
  const checkboxes = Array.from(
    document.getElementsByName("trgScalerCheckbox"),
  );

  if (checkboxes.every((checkbox) => !checkbox.checked)) {
    checkboxes.forEach((checkbox) => (checkbox.required = true));
  } else {
    checkboxes.forEach((checkbox) => (checkbox.required = false));
  }
}

function updateTrgDownloadBtnState() {
  document.getElementById("trgDownloadBtn").disabled = !document
    .getElementById("trgForm")
    .checkValidity();
}

document
  .querySelectorAll("#trgForm input, #trgForm select, #trgForm textarea")
  .forEach(function (element) {
    element.addEventListener("input", updateTrgDownloadBtnState);
  });

document
  .getElementById("trgModal")
  .addEventListener("show.bs.modal", function () {
    updateTrgMaxTimeReq();
    updateTrgScalersReq();
    updateTrgDownloadBtnState();
  });

function trgPlot() {
  updateTrgMaxTimeReq();
  updateTrgScalersReq();

  if (!document.getElementById("trgForm").checkValidity()) {
    alert("Please fix all form errors before submitting.");
    return;
  }

  const trgArgs = {
    t_bins: parseInt(document.getElementById("trgBins").value),
    t_max: parseFloat(document.getElementById("trgMaxT").value),
    t_min: parseFloat(document.getElementById("trgMinT").value),
    include_drift_veto: document.getElementById("trgDriftVeto").checked,
    include_pulser: document.getElementById("trgPulser").checked,
    include_scaledown: document.getElementById("trgScaledown").checked,
    remove_input: !document.getElementById("trgInput").checked,
    remove_output: !document.getElementById("trgOutput").checked,
  };

  newDownload({
    service: "Download",
    context: "",
    request: { TrgScalersPlot: { run_number: RUN_NUMBER, args: trgArgs } },
  });

  bootstrap.Modal.getInstance(document.getElementById("trgModal")).hide();
}

function updateVerticesMaxTimeReq() {
  document.getElementById("verticesMaxT").min =
    document.getElementById("verticesMinT").value;
}

function updateVerticesDownloadBtnState() {
  document.getElementById("verticesDownloadBtn").disabled = !document
    .getElementById("verticesForm")
    .checkValidity();
}

document
  .querySelectorAll(
    "#verticesForm input, #verticesForm select, #verticesForm textarea",
  )
  .forEach(function (element) {
    element.addEventListener("input", updateVerticesDownloadBtnState);
  });

document
  .getElementById("verticesModal")
  .addEventListener("show.bs.modal", function () {
    updateVerticesMaxTimeReq();
    updateVerticesDownloadBtnState();
  });

function verticesPlot() {
  updateVerticesMaxTimeReq();

  if (!document.getElementById("verticesForm").checkValidity()) {
    alert("Please fix all form errors before submitting.");
    return;
  }

  const verticesArgs = {
    phi_bins: parseInt(document.getElementById("verticesBinsPhi").value),
    phi_max: parseFloat(document.getElementById("verticesMaxPhi").value),
    phi_min: parseFloat(document.getElementById("verticesMinPhi").value),
    r_bins: parseInt(document.getElementById("verticesBinsR").value),
    r_max: parseFloat(document.getElementById("verticesMaxR").value),
    r_min: parseFloat(document.getElementById("verticesMinR").value),
    t_bins: parseInt(document.getElementById("verticesBinsT").value),
    t_max: parseFloat(document.getElementById("verticesMaxT").value),
    t_min: parseFloat(document.getElementById("verticesMinT").value),
    z_bins: parseInt(document.getElementById("verticesBinsZ").value),
    z_max: parseFloat(document.getElementById("verticesMaxZ").value),
    z_min: parseFloat(document.getElementById("verticesMinZ").value),
  };

  newDownload({
    service: "Download",
    context: "",
    request: { VerticesPlot: { run_number: RUN_NUMBER, args: verticesArgs } },
  });

  bootstrap.Modal.getInstance(document.getElementById("verticesModal")).hide();
}
