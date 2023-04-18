/* eslint-disable */
// https://stackoverflow.com/a/30810322
// https://stackoverflow.com/a/33928558
define([], function() {
  return function (text, container) {
    if (document.queryCommandSupported && document.queryCommandSupported('copy')) {
      var textArea = document.createElement("textarea");
      // Place in top-left corner of screen regardless of scroll position.
      textArea.style.position = 'fixed';
      textArea.style.left = '-9999px';
    
      // Ensure it has a small width and height. Setting to 1px / 1em
      // doesn't work as this gives a negative w/h on some browsers.
      textArea.style.width = '2em';
      textArea.style.height = '2em';
    
      // We don't need padding, reducing the size if it does flash render.
      textArea.style.padding = 0;
    
      // Clean up any borders.
      textArea.style.border = 'none';
      textArea.style.outline = 'none';
      textArea.style.boxShadow = 'none';
    
      // Avoid flash of white box if rendered for any reason.
      textArea.style.background = 'transparent';
      textArea.value = text;

      container.appendChild(textArea);
      textArea.focus();
      textArea.select();
      
      var successful = false;
      try {
        successful = document.execCommand('copy');
        console.log('Copying text command was ' + successful);
        return successful;
      } catch (err) {
        console.log('Oops, unable to copy');
        successful = false;
      }
      container.removeChild(textArea);
      return successful;
    }
    else {
      return false;
    }
  };
});