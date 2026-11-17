// Page chrome that mirrors docs.aws.amazon.com: guide title in the sidebar,
// breadcrumbs above the page title, and Note / Warning / Important alerts.
(function () {
    "use strict";

    var GUIDE_TITLE = "AWS Lambda Web Adapter";
    var GUIDE_SUBTITLE = "User Guide";

    var sidebar = document.querySelector("#sidebar .sidebar-scrollbox");
    var chapters = sidebar && sidebar.querySelector("ol.chapter");
    var homeLink = chapters && chapters.querySelector("a[href]");
    var homeHref = homeLink ? homeLink.getAttribute("href") : "#";

    function el(tag, className, text) {
        var node = document.createElement(tag);
        if (className) node.className = className;
        if (text) node.textContent = text;
        return node;
    }

    // Guide title block at the top of the sidebar.
    if (chapters) {
        var header = el("div", "aws-guide-header");
        var title = el("a", null, GUIDE_TITLE);
        title.href = homeHref;
        header.appendChild(title);
        header.appendChild(el("span", null, GUIDE_SUBTITLE));
        sidebar.insertBefore(header, chapters);
    }

    // Breadcrumbs: guide > part > parent chapter(s).
    function buildBreadcrumbs() {
        var active = chapters && chapters.querySelector("a.active");
        var main = document.querySelector("#content main");
        if (!active || !main) return;

        var crumbs = [{ text: GUIDE_SUBTITLE, href: homeHref }];

        // Part title: nearest preceding li.part-title among the top-level items.
        var topItem = active.closest("ol.chapter > li");
        for (var li = topItem; li; li = li.previousElementSibling) {
            if (li.classList.contains("part-title")) {
                crumbs.push({ text: li.textContent.trim() });
                break;
            }
        }

        // Parent chapters for nested pages (li > ol.section > li > a.active).
        var parents = [];
        var section = active.closest("ol.section");
        while (section) {
            var parentLi = section.parentElement && section.parentElement.previousElementSibling;
            var parentLink = parentLi && parentLi.querySelector(":scope > a[href]");
            if (parentLink) parents.unshift({ text: parentLink.textContent.trim(), href: parentLink.getAttribute("href") });
            section = section.parentElement && section.parentElement.closest("ol.section");
        }
        crumbs = crumbs.concat(parents);

        if (crumbs.length < 2 && active === homeLink) return;

        var nav = el("nav", "aws-breadcrumbs");
        nav.setAttribute("aria-label", "Breadcrumbs");
        crumbs.forEach(function (c, i) {
            if (i > 0) nav.appendChild(el("span", "sep", "›"));
            var node = el(c.href ? "a" : "span", null, c.text);
            if (c.href) node.href = c.href;
            nav.appendChild(node);
        });
        main.insertBefore(nav, main.firstChild);
    }

    // Admonitions: "> **Note:** ..." becomes an AWS-style alert with a title line.
    function styleAlerts() {
        var kinds = { note: "", tip: "", warning: "aws-warning", caution: "aws-warning", important: "aws-important" };
        document.querySelectorAll("#content main blockquote").forEach(function (quote) {
            var strong = quote.querySelector(":scope > p:first-child > strong:first-child");
            if (!strong) return;
            var match = /^(note|tip|warning|caution|important)\b:?$/i.exec(strong.textContent.trim());
            if (!match) return;
            var kind = match[1].toLowerCase();
            if (kinds[kind]) quote.classList.add(kinds[kind]);
            strong.classList.add("aws-alert-title");
            strong.textContent = match[1].charAt(0).toUpperCase() + match[1].slice(1).toLowerCase();
            // Drop the space that followed the "Note:" label.
            var next = strong.nextSibling;
            if (next && next.nodeType === Node.TEXT_NODE) next.textContent = next.textContent.replace(/^\s+/, "");
        });
    }

    buildBreadcrumbs();
    styleAlerts();
})();
