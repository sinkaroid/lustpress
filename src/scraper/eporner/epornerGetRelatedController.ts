import { load } from "cheerio";
import LustPress from "../../LustPress";
import { ISearchVideoData } from "../../interfaces";

const lust = new LustPress();

export async function scrapeContent(url: string) {
  const html = await lust.fetchBody(url);
  const $ = load(html);

  const data = $("#relateddiv .mb")
    .map((i, el) => {
      const link = $(el).find(".mbimg a").first().attr("href") || "";
      const match = link.match(/\/(video-|hd-porn\/)([^/]+)/);
      let id = "";
      if (match) {
        if (match[1] === "video-") {
          id = `video-${match[2]}`;
        } else {
          // hd-porn
          id = match[2];
        }
      }

      return {
        link: `https://www.eporner.com${link}`,
        id,
        title: $(el).find(".mbtit a").text().trim(),
        image:
          $(el).find("img").attr("data-src") || $(el).find("img").attr("src"),
        duration: $(el).find(".mbtim").text().trim(),
        rating: $(el).find(".mbrate").text().trim(),
        views: $(el).find(".mbvie").text().trim(),
        uploader: $(el).find(".mb-uploader a").text().trim(),
        video: `https://www.eporner.com/embed/${id}`,
      };
    })
    .get()
    .filter((v) => v.id && v.image);

  return {
    success: true,
    data: data as unknown as string[],
    source: url,
  } as ISearchVideoData;
}