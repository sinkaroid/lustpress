import { load } from "cheerio";
import LustPress from "../../LustPress";
import { ISearchItem } from "../../interfaces";

const lust = new LustPress();

export async function scrapeContent(url: string) {
    try {
        const res = await lust.fetchBody(url);
        const $ = load(res);

        class EPornerSearch {
            data: ISearchItem[];

            constructor() {
                this.data = $("#vidresults .mb")
                    .map((i, el) => {
                        const link =
                            $(el).find(".mbimg a").first().attr("href") || "";

                        const match = link.match(/\/(video-|hd-porn\/)([^/]+)/);

                        let id = "";

                        if (match) {
                            if (match[1] === "video-") {
                                id = `video-${match[2]}`;
                            } else {
                                id = match[2];
                            }
                        }

                        const code = id.startsWith("video-")
                            ? id.replace("video-", "")
                            : id;

                        return {
                            link: `https://www.eporner.com${link}`,
                            id,
                            title: $(el).find(".mbtit a").text().trim(),
                            image: $(el).find(".mbimg img").attr("src"),
                            duration: $(el).find(".mbtim").text().trim(),
                            rating: $(el).find(".mbrate").text().trim(),
                            views: $(el).find(".mbvie").text().trim(),
                            uploader: $(el)
                                .find(".mb-uploader a")
                                .text()
                                .trim(),
                            video: code
                                ? `https://www.eporner.com/embed/${code}/`
                                : "None",
                        };
                    })
                    .get();
            }
        }

        const ep = new EPornerSearch();
        if (ep.data.length === 0) throw Error("No result found");

        return {
            success: true,
            data: ep.data as ISearchItem[],
            source: url,
        };
    } catch (err) {
        const e = err as Error;
        throw Error(e.message);
    }
}